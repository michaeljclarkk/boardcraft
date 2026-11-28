use crate::{BoardId, ItemId};
use postgres::Transaction;
use uuid::Uuid;

pub struct Table {
    pub id: Uuid,
    pub title: String,
    pub choices: Vec<Choice>,
    pub cells: Vec<Cell>,
}

pub struct Choice {
    pub key: String,
    pub label: String,
    pub color: String,
}

pub struct Cell {
    pub item: Uuid,
    pub revision: i64,
    pub value: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    Missing,
    Invalid,
    Conflict,
    Store(postgres::Error),
}
impl From<postgres::Error> for Error {
    fn from(e: postgres::Error) -> Self {
        Self::Store(e)
    }
}

pub fn read_status(tx: &mut Transaction<'_>, board: BoardId) -> Result<Table, Error> {
    let board = board.as_uuid();
    let column = tx
        .query_opt(
            "SELECT id,title FROM status_columns WHERE board_id=$1",
            &[&board],
        )?
        .ok_or(Error::Missing)?;
    let id: Uuid = column.get(0);
    let choices = super::labels::choices(tx, id)?;
    let cells = tx
        .query(
            "SELECT i.id,coalesce(c.revision,0),c.value FROM board_items i
        JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id)
        LEFT JOIN status_cells c ON c.column_id=$2 AND c.item_id=i.id
        WHERE i.board_id=$1 AND g.active AND i.state=0
        ORDER BY g.position,g.id,i.position,i.id",
            &[&board, &id],
        )?
        .into_iter()
        .map(|r| Cell {
            item: r.get(0),
            revision: r.get(1),
            value: r.get(2),
        })
        .collect();
    Ok(Table {
        id,
        title: column.get(1),
        choices,
        cells,
    })
}

/// Lock the owning item even for a first edit (where no cell row yet exists).
/// Keep the revision on clear to reject stale edits across set/clear cycles.
pub fn set_status(
    tx: &mut Transaction<'_>,
    board: BoardId,
    column: Uuid,
    item: ItemId,
    expected: i64,
    value: Option<String>,
) -> Result<Cell, Error> {
    let board = board.as_uuid();
    let item = item.as_uuid();
    tx.query_opt("SELECT 1 FROM board_items i JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id)
        JOIN status_columns s ON s.board_id=i.board_id WHERE i.board_id=$1 AND s.id=$2 AND i.id=$3 AND g.active AND i.state=0
        FOR UPDATE OF i FOR SHARE OF g,s", &[&board,&column,&item])?.ok_or(Error::Missing)?;
    if let Some(key) = &value
        && (key == "empty"
            || tx
                .query_opt(
                    "SELECT 1 FROM status_labels WHERE column_id=$1 AND key=$2",
                    &[&column, key],
                )?
                .is_none())
    {
        return Err(Error::Invalid);
    }
    let (previous, revision) = tx
        .query_opt(
            "SELECT value,revision FROM status_cells WHERE column_id=$1 AND item_id=$2",
            &[&column, &item],
        )?
        .map_or((None, 0i64), |r| (r.get::<_, Option<String>>(0), r.get(1)));
    if revision != expected {
        return Err(Error::Conflict);
    }
    if previous == value {
        return Ok(Cell {
            item,
            revision,
            value,
        });
    }
    let revision = tx.query_one("INSERT INTO status_cells(board_id,column_id,item_id,value,revision) VALUES ($1,$2,$3,$4,1)
        ON CONFLICT(column_id,item_id) DO UPDATE SET value=excluded.value,revision=status_cells.revision+1 RETURNING revision", &[&board,&column,&item,&value])?.get(0);
    Ok(Cell {
        item,
        revision,
        value,
    })
}
