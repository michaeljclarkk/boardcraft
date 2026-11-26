use crate::fixture::Fixture;
use boardcraft_mobile_edit_client::{Credentials, EditClient};
use uuid::Uuid;

// Keep verifiers private even when a checkpoint fails (no Debug/assert_eq!).
pub struct Peer {
    pub client: EditClient,
    member: Uuid,
    primary: Vec<u8>,
    peer: Vec<u8>,
}

fn verifiers(f: &Fixture, member: Uuid) -> Vec<Vec<u8>> {
    f.connect()
        .query("SELECT account_id, token_digest FROM account_sessions", &[])
        .unwrap()
        .iter()
        .map(|row| {
            assert_eq!(row.get::<_, Uuid>(0), member);
            row.get(1)
        })
        .collect()
}

impl Peer {
    pub fn sign_in(f: &Fixture, member: Uuid, credentials: Credentials) -> Self {
        let before = verifiers(f, member);
        let [primary] = before.as_slice() else {
            panic!("expected one original session")
        };
        let client = EditClient::sign_in(f.host.port, credentials).unwrap();
        let after = verifiers(f, member);
        assert_eq!(after.len(), 2);
        assert!(after.contains(primary));
        let peer = after
            .into_iter()
            .find(|v| v != primary)
            .expect("missing peer session");
        Self {
            client,
            member,
            primary: primary.clone(),
            peer,
        }
    }

    pub fn check(&self, f: &Fixture, primary_present: bool) {
        let current = verifiers(f, self.member);
        assert_eq!(current.len(), if primary_present { 2 } else { 1 });
        assert!(current.contains(&self.peer));
        assert_eq!(current.contains(&self.primary), primary_present);
    }
}
