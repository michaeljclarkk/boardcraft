//! Actor/board scoped recovery. Drafts and submitted failures use separate documents.
use boardcraft_board::{BoardId, ItemId, RenameError};
use boardcraft_desktop::Failure;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
};
use tempfile::NamedTempFile;
use uuid::Uuid;

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const INVALID: &str =
    "Local name recovery is unreadable. Check the recovery directory before retrying.";
const CHANGED: &str = "Local name recovery changed in another window. Reopen the board.";

#[derive(Serialize, Deserialize)]
enum Reason {
    Blank,
    TooLong,
    Missing,
    Conflict(String),
    Database,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    token: Uuid,
    raw: String,
    reason: Reason,
}

impl Record {
    pub fn from_error(raw: String, reason: &Failure) -> Self {
        let reason = match reason {
            Failure::Rejected(RenameError::BlankTitle) => Reason::Blank,
            Failure::Rejected(RenameError::TitleTooLong) => Reason::TooLong,
            Failure::Rejected(RenameError::MissingItem(_)) => Reason::Missing,
            Failure::Conflict(current) => Reason::Conflict(current.clone()),
            Failure::Database => Reason::Database,
        };
        Self {
            token: Uuid::new_v4(),
            raw,
            reason,
        }
    }

    pub fn into_failure(self, item: ItemId) -> (Uuid, String, Failure) {
        let reason = match self.reason {
            Reason::Blank => Failure::Rejected(RenameError::BlankTitle),
            Reason::TooLong => Failure::Rejected(RenameError::TitleTooLong),
            Reason::Missing => Failure::Rejected(RenameError::MissingItem(item)),
            Reason::Conflict(current) => Failure::Conflict(current),
            Reason::Database => Failure::Database,
        };
        (self.token, self.raw, reason)
    }
}

pub trait LocalRecord: Serialize + serde::de::DeserializeOwned {
    const FILE: &'static str;
    fn token(&self) -> Uuid;
    fn valid(&self) -> bool {
        !self.token().is_nil()
    }
}

impl LocalRecord for Record {
    const FILE: &'static str = "names.json";
    fn token(&self) -> Uuid {
        self.token
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub token: Uuid,
    pub raw: String,
    pub entry: String,
    pub revision: i64,
}

impl LocalRecord for Draft {
    const FILE: &'static str = "drafts.json";
    fn token(&self) -> Uuid {
        self.token
    }
    fn valid(&self) -> bool {
        !self.token.is_nil() && self.revision > 0
    }
}

type Records<R> = BTreeMap<Uuid, R>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound = "R: LocalRecord")]
struct Document<R> {
    version: u8,
    actor: Uuid,
    board: Uuid,
    #[serde(deserialize_with = "unique_names")]
    names: Records<R>,
}

// BTreeMap's default decoder silently replaces duplicate keys. Reject ambiguous
// files rather than dropping a failed request on the next unrelated write.
fn unique_names<'de, R: LocalRecord, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Records<R>, D::Error> {
    struct Names<R>(std::marker::PhantomData<R>);
    impl<'de, R: LocalRecord> serde::de::Visitor<'de> for Names<R> {
        type Value = Records<R>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("one local name record per item UUID")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut names = Records::new();
            while let Some((id, record)) = a.next_entry()? {
                if names.insert(id, record).is_some() {
                    return Err(serde::de::Error::custom("duplicate item"));
                }
            }
            Ok(names)
        }
    }
    d.deserialize_map(Names(std::marker::PhantomData))
}

#[derive(Clone)]
pub struct Recovery {
    dir: PathBuf,
    actor: Uuid,
    board: Uuid,
}

impl Recovery {
    pub fn configured(board: BoardId) -> Result<Option<Self>, &'static str> {
        let Some(actor) = std::env::var_os("BOARDCRAFT_LOCAL_ACTOR") else {
            return Ok(None);
        };
        let actor = actor
            .to_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .filter(|u| !u.is_nil())
            .ok_or("BOARDCRAFT_LOCAL_ACTOR must be a non-nil UUID.")?;
        let root = std::env::var_os("BOARDCRAFT_RECOVERY_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join("boardcraft/name-recovery"))
            })
            .filter(|p| p.is_absolute())
            .ok_or("Set BOARDCRAFT_RECOVERY_DIR to an absolute local directory.")?;
        Ok(Some(Self {
            dir: root
                .join(actor.to_string())
                .join(board.as_uuid().to_string()),
            actor,
            board: board.as_uuid(),
        }))
    }

    fn document<R: LocalRecord>(&self) -> Result<Document<R>, &'static str> {
        let file = match File::open(self.dir.join(R::FILE)) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Document {
                    version: 1,
                    actor: self.actor,
                    board: self.board,
                    names: Records::new(),
                });
            }
            Err(_) => return Err(INVALID),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| INVALID)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(INVALID);
        }
        let doc: Document<R> = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
        if doc.version != 1
            || doc.actor != self.actor
            || doc.board != self.board
            || doc.names.iter().any(|(id, r)| id.is_nil() || !r.valid())
        {
            return Err(INVALID);
        }
        Ok(doc)
    }

    pub fn load<R: LocalRecord>(&self) -> Result<Records<R>, &'static str> {
        Ok(self.document()?.names)
    }

    // All writers hold the same OS lock across read/compare/atomic replacement.
    // Compare the occurrence, not the raw value: identical failures can recur.
    pub fn replace<R: LocalRecord>(
        &self,
        item: ItemId,
        expected: Option<Uuid>,
        record: Option<R>,
    ) -> Result<Option<Uuid>, &'static str> {
        let io_error =
            "Cannot update local name recovery. Check the recovery directory and reopen.";
        fs::create_dir_all(&self.dir).map_err(|_| io_error)?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.dir.join("names.lock"))
            .map_err(|_| io_error)?;
        lock.lock().map_err(|_| io_error)?;
        let mut doc = self.document::<R>()?;
        let id = item.as_uuid();
        if doc.names.get(&id).map(LocalRecord::token) != expected {
            return Err(CHANGED);
        }
        let token = record.as_ref().map(LocalRecord::token);
        if let Some(record) = record {
            doc.names.insert(id, record);
        } else {
            doc.names.remove(&id);
        }
        let bytes = serde_json::to_vec(&doc).map_err(|_| io_error)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Local name recovery is full; the previous data was preserved.");
        }
        let mut temp = NamedTempFile::new_in(&self.dir).map_err(|_| io_error)?;
        temp.write_all(&bytes)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|_| io_error)?;
        temp.persist(self.dir.join(R::FILE)).map_err(|_| io_error)?;
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(dir: &tempfile::TempDir) -> Recovery {
        Recovery {
            dir: dir.path().to_owned(),
            actor: Uuid::new_v4(),
            board: Uuid::new_v4(),
        }
    }

    #[test]
    fn stale_dismissal_preserves_identical_new_failure_and_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let store = fixture(&dir);
        let item = ItemId::new(Uuid::from_u128(17)).unwrap();
        let sibling = ItemId::new(Uuid::from_u128(42)).unwrap();
        let failed = || Some(Record::from_error(" Coral ".into(), &Failure::Database));
        let first = store.replace(item, None, failed()).unwrap();
        let other = store.replace(sibling, None, failed()).unwrap();
        let second = store.replace(item, first, failed()).unwrap();
        assert_eq!(store.replace::<Record>(item, first, None), Err(CHANGED));
        let records = store.load::<Record>().unwrap();
        assert_eq!(Some(records[&item.as_uuid()].token), second);
        assert_eq!(Some(records[&sibling.as_uuid()].token), other);
    }

    #[test]
    fn unreadable_or_wrong_scope_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let store = fixture(&dir);
        let path = dir.path().join("names.json");
        let item = ItemId::new(Uuid::from_u128(17)).unwrap();
        let wrong = serde_json::to_vec(&Document::<Record> {
            version: 1,
            board: store.board,
            actor: Uuid::new_v4(),
            names: Records::new(),
        })
        .unwrap();
        for bytes in [b"broken".to_vec(), wrong] {
            fs::write(&path, &bytes).unwrap();
            assert!(store.load::<Record>().is_err());
            assert_eq!(
                store.replace(
                    item,
                    None,
                    Some(Record::from_error("   ".into(), &Failure::Database))
                ),
                Err(INVALID)
            );
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn stale_escape_keeps_newer_draft_even_with_identical_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = fixture(&dir);
        let item = ItemId::new(Uuid::from_u128(17)).unwrap();
        let draft = || {
            Some(Draft {
                token: Uuid::new_v4(),
                raw: "".into(),
                entry: "Review".into(),
                revision: 1,
            })
        };
        let original = store.replace(item, None, draft()).unwrap();
        let newer = store.replace(item, original, draft()).unwrap();
        assert_eq!(store.replace::<Draft>(item, original, None), Err(CHANGED));
        assert_eq!(
            Some(store.load::<Draft>().unwrap()[&item.as_uuid()].token),
            newer
        );
    }
}
