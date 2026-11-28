use super::{Choice, Error};
use crate::BoardId;
use postgres::Transaction;
use uuid::Uuid;

pub fn read_status_labels(
    tx: &mut Transaction<'_>,
    board: BoardId,
    column: Uuid,
) -> Result<Vec<Choice>, Error> {
    tx.query_opt(
        "SELECT 1 FROM status_columns WHERE board_id=$1 AND id=$2 FOR SHARE",
        &[&board.as_uuid(), &column],
    )?
    .ok_or(Error::Missing)?;
    choices(tx, column)
}

pub(super) fn choices(tx: &mut Transaction<'_>, column: Uuid) -> Result<Vec<Choice>, Error> {
    Ok(tx
        .query(
            "SELECT key,label,color FROM status_labels WHERE column_id=$1 ORDER BY position",
            &[&column],
        )?
        .into_iter()
        .map(|r| Choice {
            key: r.get(0),
            label: r.get(1),
            color: r.get(2),
        })
        .collect())
}

/// The source patches fields independently and preserves label identities, cell
/// values, cell revisions and positions. Retrying the same patch is a no-op.
pub fn save_status_label(
    tx: &mut Transaction<'_>,
    board: BoardId,
    column: Uuid,
    key: &str,
    label: Option<&str>,
    color: Option<&str>,
) -> Result<Choice, Error> {
    let valid_key = matches!(key, "working" | "done" | "stuck" | "empty")
        || key
            .strip_prefix("l-")
            .is_some_and(|id| Uuid::parse_str(id).is_ok());
    if !valid_key
        || (label.is_none() && color.is_none())
        || label.is_some_and(|s| s.encode_utf16().count() > 160)
        || color.is_some_and(|s| {
            s.len() != 7 || !s.starts_with('#') || !s[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err(Error::Invalid);
    }
    tx.query_opt(
        "SELECT 1 FROM boards WHERE id=$1 FOR UPDATE",
        &[&board.as_uuid()],
    )?
    .ok_or(Error::Missing)?;
    tx.query_opt(
        "SELECT 1 FROM status_columns WHERE board_id=$1 AND id=$2 FOR UPDATE",
        &[&board.as_uuid(), &column],
    )?
    .ok_or(Error::Missing)?;
    let existing = tx.query_opt(
        "SELECT label,color FROM status_labels WHERE column_id=$1 AND key=$2",
        &[&column, &key],
    )?;
    let (label, color) = if let Some(row) = existing {
        (
            label.unwrap_or(row.get(0)).to_owned(),
            color.unwrap_or(row.get(1)).to_owned(),
        )
    } else {
        let (Some(label), Some(color)) = (label, color) else {
            return Err(Error::Invalid);
        };
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM status_labels WHERE column_id=$1",
                &[&column],
            )?
            .get(0);
        if count >= 250 {
            return Err(Error::Invalid);
        }
        (label.to_owned(), color.to_owned())
    };
    let changed=tx.execute("INSERT INTO status_labels(column_id,key,label,color,position)
        SELECT $1,$2,$3,$4,coalesce(max(position)+1,0) FROM status_labels WHERE column_id=$1
        ON CONFLICT(column_id,key) DO UPDATE SET label=excluded.label,color=excluded.color
        WHERE (status_labels.label,status_labels.color) IS DISTINCT FROM (excluded.label,excluded.color)",
        &[&column,&key,&label,&color])?;
    if changed > 0 {
        tx.execute(
            "UPDATE status_columns SET revision=revision+1 WHERE id=$1",
            &[&column],
        )?;
        tx.execute(
            "UPDATE boards SET revision=revision+1 WHERE id=$1",
            &[&board.as_uuid()],
        )?;
    }
    Ok(Choice {
        key: key.to_owned(),
        label,
        color,
    })
}
