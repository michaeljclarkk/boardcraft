use boardcraft_account::{Role, Token, sign_in};
use boardcraft_mobile_edit_client::EditClient;
use ring::pbkdf2;
use std::num::NonZeroU32;
use uuid::Uuid;

use crate::fixture::Fixture;

pub struct Caller {
    pub id: Uuid,
    pub token: Token,
}

impl Caller {
    pub fn sign_in(f: &Fixture, role: Role) -> Self {
        let id = Uuid::new_v4();
        let login = id.simple().to_string();
        let password = Uuid::new_v4().to_string();
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
        f.connect().execute(
            "INSERT INTO accounts(id, username, email, role, active, password_salt, password_verifier)
             VALUES ($1,$2,$3,$4,true,$5,$6)",
            &[&id, &login, &format!("{login}@example.invalid"), &role, &&salt.as_bytes()[..], &&verifier[..]],
        ).unwrap_or_else(|_| panic!("fixture account insert failed"));
        let token = sign_in(&f.schema.config, &login, &password).unwrap();
        Self { id, token }
    }

    pub fn board_creator(f: &Fixture) -> Self {
        let caller = Self::sign_in(f, Role::Member);
        f.connect()
            .execute(
                "INSERT INTO open_workspace_private_boards(board_id,creator_id) VALUES ($1,$2)",
                &[&f.board, &caller.id],
            )
            .unwrap();
        caller
    }

    #[allow(
        dead_code,
        reason = "the emulator test constructs the client on the device"
    )]
    pub fn client(&self, port: u16) -> EditClient {
        EditClient::with_bearer(port, self.token.encoded()).unwrap()
    }
}
