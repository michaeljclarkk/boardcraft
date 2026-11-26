use crate::fixture::Fixture;
use uuid::Uuid;

// No Debug and no formatted comparisons: the verifier stays in this process.
#[derive(PartialEq)]
pub struct StoredSession {
    digest: Vec<u8>,
    member: Uuid,
}

impl StoredSession {
    #[cfg(test)]
    pub fn present(&self, f: &Fixture) -> bool {
        f.connect()
            .query_opt(
                "SELECT 1 FROM account_sessions WHERE account_id=$1 AND token_digest=$2",
                &[&self.member, &self.digest],
            )
            .unwrap()
            .is_some()
    }
    pub fn all(f: &Fixture, member: Uuid) -> Vec<Self> {
        f.connect()
            .query(
                "SELECT token_digest, account_id FROM account_sessions ORDER BY token_digest",
                &[],
            )
            .unwrap()
            .iter()
            .map(|row| {
                assert_eq!(row.get::<_, Uuid>(1), member);
                Self {
                    digest: row.get(0),
                    member,
                }
            })
            .collect()
    }
    pub fn only(f: &Fixture, member: Uuid) -> Self {
        let mut sessions = Self::all(f, member);
        assert_eq!(sessions.len(), 1);
        sessions.pop().unwrap()
    }

    pub fn revoke(&self, f: &Fixture) {
        let deleted = f
            .connect()
            .execute(
                "DELETE FROM account_sessions WHERE token_digest=$1 AND account_id=$2",
                &[&self.digest, &self.member],
            )
            .expect("could not revoke original session");
        assert_eq!(deleted, 1);
        assert_no_session(f);
    }
}

pub fn assert_no_session(f: &Fixture) {
    let count: i64 = f
        .connect()
        .query_one("SELECT count(*) FROM account_sessions", &[])
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
}
