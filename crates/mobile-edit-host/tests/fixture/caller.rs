use boardcraft_account::{Role, Token, sign_in};
use boardcraft_mobile_edit_client::EditClient;
use uuid::Uuid;

use crate::fixture::Fixture;

pub struct Caller {
    pub id: Uuid,
    pub token: Token,
}

impl Caller {
    pub fn sign_in(f: &Fixture, role: Role) -> Self {
        let login = Uuid::new_v4().simple().to_string();
        let password = Uuid::new_v4().to_string();
        let id = f.seed_account(&login, &password, role);
        let token = sign_in(&f.schema.config, &login, &password).unwrap();
        Self { id, token }
    }

    pub fn board_creator(f: &Fixture) -> Self {
        let caller = Self::sign_in(f, Role::Member);
        f.set_creator(caller.id);
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
