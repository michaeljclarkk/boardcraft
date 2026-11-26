use crate::fixture::Fixture;
use uuid::Uuid;

// No Debug and no formatted comparisons: the verifier stays in this process.
#[derive(PartialEq)]
pub struct StoredSession {
    digest: Vec<u8>,
    member: Uuid,
}

impl StoredSession {
    pub fn only(f: &Fixture, member: Uuid) -> Self {
        let row = f
            .connect()
            .query_one("SELECT token_digest, account_id FROM account_sessions", &[])
            .expect("expected exactly one session");
        let session = Self {
            digest: row.get(0),
            member: row.get(1),
        };
        assert_eq!(session.member, member);
        session
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
