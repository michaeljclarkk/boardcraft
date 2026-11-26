-- A policy row models ONLY a private board in an open workspace. Migrating an
-- existing board does not classify it or grant anybody access.
CREATE TABLE IF NOT EXISTS open_workspace_private_boards (
    board_id uuid PRIMARY KEY REFERENCES boards(id) ON DELETE CASCADE,
    creator_id uuid NOT NULL REFERENCES accounts(id)
);
CREATE TABLE IF NOT EXISTS private_board_subscribers (
    board_id uuid NOT NULL REFERENCES open_workspace_private_boards(board_id) ON DELETE CASCADE,
    account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    PRIMARY KEY (board_id, account_id)
);
