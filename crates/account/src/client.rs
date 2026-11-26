use super::{Account, Error, Token, resolve, revoke, sign_in};
use postgres::Config;

/// The identifier field is required, but only credential submission validates its contents.
pub struct PasswordStep(String);

impl PasswordStep {
    pub fn advance(raw: &str) -> Result<Self, Error> {
        if raw.is_empty() {
            return Err(Error::Rejected);
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn submit(
        &self,
        config: &Config,
        password: &str,
        session: &mut SessionClient,
    ) -> Result<(), Error> {
        if session.token.is_some() {
            return Err(Error::Rejected);
        }
        session.token = Some(sign_in(config, &self.0, password)?);
        session.resume(config)
    }
}

/// Only successful resolution sets `ready`. A transport/store fault leaves just the token.
#[derive(Default)]
pub struct SessionClient {
    token: Option<Token>,
    ready: Option<Account>,
}

impl SessionClient {
    pub fn reopen(token: Token) -> Self {
        Self {
            token: Some(token),
            ready: None,
        }
    }
    pub fn token(&self) -> Option<&Token> {
        self.token.as_ref()
    }
    pub fn ready(&self) -> Option<&Account> {
        self.ready.as_ref()
    }

    pub fn resume(&mut self, config: &Config) -> Result<(), Error> {
        self.ready = None;
        let token = self.token.as_ref().ok_or(Error::Rejected)?;
        match resolve(config, token) {
            Ok(account) => {
                self.ready = Some(account);
                Ok(())
            }
            Err(error) => {
                if error == Error::Rejected {
                    self.token = None;
                }
                Err(error)
            }
        }
    }

    pub fn logout(&mut self, config: &Config) -> Result<(), Error> {
        self.ready = None;
        if let Some(token) = &self.token {
            revoke(config, token)?;
        }
        self.token = None;
        Ok(())
    }
}
