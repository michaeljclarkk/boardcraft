-- An untouched preset has revision 1 and no folds. Materialize it on first save.
CREATE TABLE main_tables (
    board_id uuid PRIMARY KEY REFERENCES boards(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision > 0)
);
CREATE TABLE main_table_folds (
    board_id uuid NOT NULL REFERENCES main_tables(board_id) ON DELETE CASCADE,
    group_id text COLLATE "C" NOT NULL,
    PRIMARY KEY (board_id, group_id),
    FOREIGN KEY (board_id, group_id) REFERENCES board_groups(board_id, id) ON DELETE CASCADE
);
