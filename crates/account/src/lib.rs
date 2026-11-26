//! Blocking operations for a trusted local caller. There is no account creation path.
mod client;
pub use client::{PasswordStep, SessionClient};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use postgres::{Config, NoTls};
use ring::{
    digest, pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use std::{fmt, num::NonZeroU32};
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Rejected,
    Unavailable,
}

fn unavailable<E>(operation: &'static str) -> impl FnOnce(E) -> Error {
    move |_| {
        // Database diagnostics can include parameter/row values.
        log::error!("account {operation} unavailable");
        Error::Unavailable
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Viewer,
    Member,
}

impl Role {
    /// A role restriction, not an authorization check for any particular board.
    pub fn is_read_only(self) -> bool {
        self == Self::Viewer
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub role: Role,
}

#[derive(Clone)]
pub struct Token(String);

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token([redacted])")
    }
}

impl Token {
    /// Decode the caller's in-memory representation on reconnect. Only `resolve` authenticates it.
    pub fn from_encoded(raw: &str) -> Result<Self, Error> {
        if raw.len() != 43 || URL_SAFE_NO_PAD.decode_slice(raw, &mut [0; 32]).is_err() {
            return Err(Error::Rejected);
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn encoded(&self) -> &str {
        &self.0
    }

    fn issue() -> Result<Self, Error> {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(unavailable("issue"))?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    fn digest(&self) -> digest::Digest {
        digest::digest(&digest::SHA256, self.0.as_bytes())
    }
}

pub fn migrate(config: &Config) -> Result<(), Error> {
    let mut db = config.connect(NoTls).map_err(unavailable("migrate"))?;
    db.batch_execute(concat!(
        include_str!("../migrations/0001_account_session.sql"),
        "\n",
        include_str!("../migrations/0002_owner.sql")
    ))
    .map_err(unavailable("migrate"))
}

fn identifier(raw: &str) -> Result<&str, Error> {
    if raw.encode_utf16().take(321).count() > 320 || raw.trim().is_empty() || raw.contains('\0') {
        Err(Error::Rejected)
    } else {
        Ok(raw.trim())
    }
}

pub fn sign_in(config: &Config, login: &str, password: &str) -> Result<Token, Error> {
    let login = identifier(login)?;
    if password.is_empty() || password.encode_utf16().take(1025).count() > 1024 {
        return Err(Error::Rejected);
    }
    let mut db = config.connect(NoTls).map_err(unavailable("sign-in"))?;
    let mut tx = db.transaction().map_err(unavailable("sign-in"))?;
    // Hold the account stable through commit. Reject ambiguous cross-column matches.
    let rows = tx
        .query(
            "SELECT id, active, password_salt, password_verifier FROM accounts
        WHERE lower(username) = lower($1) OR lower(email) = lower($1) LIMIT 2 FOR SHARE",
            &[&login],
        )
        .map_err(unavailable("sign-in"))?;
    let credentials = match rows.as_slice() {
        [row] if row.get::<_, bool>(1) => row
            .get::<_, Option<&[u8]>>(2)
            .zip(row.get::<_, Option<&[u8]>>(3))
            .map(|(salt, hash)| (row.get::<_, Uuid>(0), salt, hash)),
        _ => None,
    };
    // Perform the same password work for an unknown, disabled or passwordless login.
    let (salt, hash) = credentials
        .map(|(_, s, h)| (s, h))
        .unwrap_or((&[0; 16], &[0; 32]));
    pbkdf2::verify(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(600_000).unwrap(),
        salt,
        password.as_bytes(),
        hash,
    )
    .map_err(|_| Error::Rejected)?;
    let (id, _, _) = credentials.ok_or(Error::Rejected)?;
    let token = Token::issue()?;
    tx.execute(
        "INSERT INTO account_sessions (account_id, token_digest) VALUES ($1, $2)",
        &[&id, &token.digest().as_ref()],
    )
    .map_err(unavailable("sign-in"))?;
    tx.commit().map_err(unavailable("sign-in"))?;
    Ok(token)
}

pub fn resolve(config: &Config, token: &Token) -> Result<Account, Error> {
    let mut db = config.connect(NoTls).map_err(unavailable("resolve"))?;
    resolve_on(&mut db, token)
}

/// Within a protected operation's transaction, keep the session and current role
/// stable until the caller finishes using this decision.
pub fn resolve_on(db: &mut impl postgres::GenericClient, token: &Token) -> Result<Account, Error> {
    let row = db
        .query_opt(
            "SELECT a.id, a.role FROM account_sessions s JOIN accounts a ON a.id = s.account_id
        WHERE s.token_digest = $1 AND a.active FOR SHARE OF a, s",
            &[&token.digest().as_ref()],
        )
        .map_err(unavailable("resolve"))?
        .ok_or(Error::Rejected)?;
    let role = match row.get::<_, &str>(1) {
        "viewer" => Role::Viewer,
        "owner" => Role::Owner,
        "member" => Role::Member,
        _ => return Err(unavailable("resolve")(())),
    };
    Ok(Account {
        id: row.get(0),
        role,
    })
}

/// Idempotent single-token revocation (including already disabled accounts).
pub fn revoke(config: &Config, token: &Token) -> Result<(), Error> {
    let mut db = config.connect(NoTls).map_err(unavailable("revoke"))?;
    db.execute(
        "DELETE FROM account_sessions WHERE token_digest = $1",
        &[&token.digest().as_ref()],
    )
    .map_err(unavailable("revoke"))?;
    Ok(())
}
