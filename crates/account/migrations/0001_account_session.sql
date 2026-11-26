CREATE TABLE IF NOT EXISTS accounts (
    id uuid PRIMARY KEY,
    username text NOT NULL,
    email text NOT NULL,
    role text NOT NULL CHECK (role IN ('viewer', 'member')),
    active boolean NOT NULL,
    -- Version 1: PBKDF2-HMAC-SHA256, 600000 rounds. Both NULL for passwordless accounts.
    password_salt bytea CHECK (octet_length(password_salt) = 16),
    password_verifier bytea CHECK (octet_length(password_verifier) = 32),
    CHECK ((password_salt IS NULL) = (password_verifier IS NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS accounts_username_key ON accounts (lower(username));
CREATE UNIQUE INDEX IF NOT EXISTS accounts_email_key ON accounts (lower(email));

CREATE TABLE IF NOT EXISTS account_sessions (
    token_digest bytea PRIMARY KEY CHECK (octet_length(token_digest) = 32),
    account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
);
