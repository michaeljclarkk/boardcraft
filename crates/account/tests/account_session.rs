use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use boardcraft_account::{
    self as account, Account, Error, PasswordStep, Role, SessionClient, Token,
};
use postgres::{Client, Config, NoTls};
use ring::{
    digest, pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use std::{num::NonZeroU32, time::Duration};
use uuid::Uuid;

// Do not include DB diagnostics, verifier bytes, passwords or bearer values in test output.
#[track_caller]
fn checked<T, E>(result: Result<T, E>) -> T {
    result.ok().expect("account fixture operation failed")
}

struct Fixture {
    config: Config,
    schema: String,
    id: Uuid,
    password: String,
}

impl Fixture {
    fn new() -> Self {
        let url = std::env::var("RUSTDAY_TEST_DATABASE_URL")
            .expect("RUSTDAY_TEST_DATABASE_URL is required");
        let mut config: Config = checked(url.parse());
        assert!(
            config.get_dbname() == Some("rustday_test"),
            "expected explicit rustday_test"
        );
        let schema = format!("account_test_{}", Uuid::new_v4().simple());
        config
            .connect_timeout(Duration::from_secs(5))
            .options(&format!(
                "-csearch_path={schema} -cstatement_timeout=10000 -clock_timeout=5000"
            ));
        let mut db = checked(config.connect(NoTls));
        checked(db.batch_execute(&format!("CREATE SCHEMA {schema}")));
        // 1024 UTF-16 units, generated in memory, with meaningful surrounding spaces.
        let password = format!(" {}{} ", Uuid::new_v4().simple(), "🌱".repeat(495));
        let fixture = Self {
            config,
            schema,
            id: Uuid::new_v4(),
            password,
        };
        checked(account::migrate(&fixture.config));
        let mut salt = [0u8; 16];
        checked(SystemRandom::new().fill(&mut salt));
        let mut verifier = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(600_000).unwrap(),
            &salt,
            fixture.password.as_bytes(),
            &mut verifier,
        );
        checked(db.execute("INSERT INTO accounts(id, username, email, role, active, password_salt, password_verifier)
            VALUES ($1,'Reed','reed@example.invalid','viewer',true,$2,$3)", &[&fixture.id, &&salt[..], &&verifier[..]]));
        fixture
    }

    fn db(&self) -> Client {
        checked(self.config.connect(NoTls))
    }

    fn sql(&self, sql: &str) {
        checked(self.db().batch_execute(sql));
    }

    fn check_password(&self) {
        let row = checked(self.db().query_one(
            "SELECT password_salt, password_verifier FROM accounts WHERE id=$1",
            &[&self.id],
        ));
        let salt: &[u8] = row.get(0);
        let verifier: &[u8] = row.get(1);
        assert!(salt.len() == 16 && verifier.len() == 32);
        checked(pbkdf2::verify(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(600_000).unwrap(),
            salt,
            self.password.as_bytes(),
            verifier,
        ));
    }

    /// Fresh connection; independently digest the encoded (not decoded) token.
    fn sessions(&self, tokens: &[&Token]) {
        let mut db = self.db();
        let rows = checked(db.query("SELECT account_id, token_digest FROM account_sessions", &[]));
        assert_eq!(rows.len(), tokens.len());
        for token in tokens {
            assert!(checked(URL_SAFE_NO_PAD.decode(token.encoded())).len() == 32);
            let expected = digest::digest(&digest::SHA256, token.encoded().as_bytes());
            assert!(rows.iter().any(
                |r| r.get::<_, Uuid>(0) == self.id && r.get::<_, &[u8]>(1) == expected.as_ref()
            ));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let result = self
            .config
            .connect(NoTls)
            .and_then(|mut db| db.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema)));
        if result.is_err() && std::thread::panicking() {
            eprintln!("also failed to clean up account fixture");
        } else {
            checked(result);
        }
    }
}

#[test]
fn rejected_credentials_then_sign_in() {
    let f = Fixture::new();
    let mut client = SessionClient::default();
    let unknown = PasswordStep::advance("Missing").unwrap();
    assert_eq!(
        unknown.submit(&f.config, &f.password, &mut client),
        Err(Error::Rejected)
    );
    // The password step sends the original input. Its raw limit precedes normalization.
    let login = format!("{}rEEd ", " ".repeat(315));
    let step = PasswordStep::advance(&login).unwrap();
    assert!(matches!(PasswordStep::advance(""), Err(Error::Rejected)));
    for bad in [" \t", &format!("{login} "), &"🌱".repeat(161)] {
        let invalid = PasswordStep::advance(bad).unwrap();
        assert_eq!(
            invalid.submit(&f.config, &f.password, &mut client),
            Err(Error::Rejected)
        );
        assert!(matches!(
            account::sign_in(&f.config, bad, &f.password),
            Err(Error::Rejected)
        ));
    }
    for bad in [f.password.trim(), "", &format!("{} ", f.password)] {
        assert_eq!(
            step.submit(&f.config, bad, &mut client),
            Err(Error::Rejected)
        );
    }
    f.sql("UPDATE accounts SET active=false");
    assert_eq!(
        step.submit(&f.config, &f.password, &mut client),
        Err(Error::Rejected)
    );
    f.sql(
        "UPDATE accounts SET active=true;
           INSERT INTO accounts (id, username, email, role, active) VALUES
           ('00000000-0000-0000-0000-000000000011','Fern','fern@example.invalid','viewer',true)",
    );
    assert_eq!(
        PasswordStep::advance("Fern")
            .unwrap()
            .submit(&f.config, &f.password, &mut client),
        Err(Error::Rejected)
    );
    let count: i64 = checked(f.db().query_one("SELECT count(*) FROM accounts", &[])).get(0);
    assert_eq!(count, 2);
    assert!(client.token().is_none() && client.ready().is_none());
    f.sessions(&[]);

    // The insert succeeds but deferred constraint checking prevents commit.
    f.sql(
        "CREATE TABLE allowed_fixture_accounts (id uuid PRIMARY KEY);
           ALTER TABLE account_sessions ADD CONSTRAINT fixture_commit FOREIGN KEY (account_id)
           REFERENCES allowed_fixture_accounts(id) DEFERRABLE INITIALLY DEFERRED",
    );
    assert_eq!(
        step.submit(&f.config, &f.password, &mut client),
        Err(Error::Unavailable)
    );
    assert!(client.token().is_none() && client.ready().is_none());
    f.sessions(&[]);
    f.sql("ALTER TABLE account_sessions DROP CONSTRAINT fixture_commit");

    step.submit(&f.config, &f.password, &mut client).unwrap();
    let viewer = Account {
        id: f.id,
        role: Role::Viewer,
    };
    assert_eq!(client.ready(), Some(&viewer));
    assert!(client.ready().unwrap().role.is_read_only());
    let token = client.token().unwrap();
    assert_eq!(account::resolve(&f.config, token).unwrap(), viewer);
    f.check_password();
    f.sessions(&[token]);
    let email = account::sign_in(&f.config, " REED@Example.Invalid\t", &f.password).unwrap();
    assert_eq!(account::resolve(&f.config, &email).unwrap(), viewer);
    f.sessions(&[token, &email]);
}

#[test]
fn resolve_retry_and_revoke() {
    let f = Fixture::new();
    let step = PasswordStep::advance("Reed").unwrap();
    let mut first = SessionClient::default();
    // Login can commit even if the subsequent role lookup is temporarily unavailable.
    f.sql("ALTER TABLE accounts RENAME COLUMN role TO unavailable_role");
    assert_eq!(
        step.submit(&f.config, &f.password, &mut first),
        Err(Error::Unavailable)
    );
    assert!(first.ready().is_none());
    let encoded = first.token().unwrap().encoded().to_owned();
    drop(first);
    let old = Token::from_encoded(&encoded).unwrap();
    assert!(matches!(Token::from_encoded(""), Err(Error::Rejected)));
    f.sessions(&[&old]);
    let mut reopened = SessionClient::reopen(old.clone());
    assert!(reopened.ready().is_none());
    assert_eq!(reopened.resume(&f.config), Err(Error::Unavailable));
    assert!(reopened.token().is_some() && reopened.ready().is_none());
    f.sql("ALTER TABLE accounts RENAME COLUMN unavailable_role TO role");
    reopened.resume(&f.config).unwrap();
    assert_eq!(
        reopened.ready(),
        Some(&Account {
            id: f.id,
            role: Role::Viewer
        })
    );
    f.sql("UPDATE accounts SET role='member'");
    reopened.resume(&f.config).unwrap();
    let member = Account {
        id: f.id,
        role: Role::Member,
    };
    assert_eq!(reopened.ready(), Some(&member));

    let mut second = SessionClient::default();
    step.submit(&f.config, &f.password, &mut second).unwrap();
    let other = second.token().unwrap().clone();
    assert!(old.encoded() != other.encoded());
    f.sessions(&[&old, &other]);
    reopened.logout(&f.config).unwrap();
    assert!(reopened.ready().is_none() && reopened.token().is_none());
    f.sessions(&[&other]);
    let mut stale = SessionClient::reopen(old);
    assert_eq!(stale.resume(&f.config), Err(Error::Rejected));
    assert!(stale.token().is_none() && stale.ready().is_none());
    second.resume(&f.config).unwrap();
    assert_eq!(second.ready(), Some(&member));
    f.sql("UPDATE accounts SET active=false");
    assert_eq!(second.resume(&f.config), Err(Error::Rejected));
    assert!(second.token().is_none() && second.ready().is_none());
    f.sessions(&[&other]);
}
