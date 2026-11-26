use super::Fixture;
use boardcraft_account::Role;
use ring::pbkdf2;
use std::num::NonZeroU32;
use uuid::Uuid;

/// Seed credentials without creating a session. Never expose SQL diagnostics for this insert.
pub fn seed_account_on(db: &mut postgres::Client, login: &str, password: &str, role: Role) -> Uuid {
    let id = Uuid::new_v4();
    let salt = Uuid::new_v4();
    let mut verifier = [0; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(600_000).unwrap(),
        salt.as_bytes(),
        password.as_bytes(),
        &mut verifier,
    );
    let role = match role {
        Role::Viewer => "viewer",
        Role::Member => "member",
    };
    db.execute(
        "INSERT INTO accounts(id, username, email, role, active, password_salt, password_verifier)
             VALUES ($1,$2,$3,$4,true,$5,$6)",
        &[
            &id,
            &login,
            &format!("{login}@example.invalid"),
            &role,
            &&salt.as_bytes()[..],
            &&verifier[..],
        ],
    )
    .unwrap_or_else(|_| panic!("fixture account insert failed"));
    id
}

impl Fixture {
    pub fn seed_account(&self, login: &str, password: &str, role: Role) -> Uuid {
        super::seed_account_on(&mut self.connect(), login, password, role)
    }

    pub fn set_creator(&self, id: Uuid) {
        self.connect()
            .execute(
                "INSERT INTO board_policies(board_id,creator_id) VALUES ($1,$2)",
                &[&self.board, &id],
            )
            .unwrap();
    }
}
