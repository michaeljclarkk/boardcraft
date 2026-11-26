CREATE TABLE board_groups (
    board_id uuid NOT NULL REFERENCES boards(id),
    id text COLLATE "C" NOT NULL CHECK (id <> ''),
    name text NOT NULL,
    position bigint NOT NULL CHECK (position >= 0),
    active boolean NOT NULL,
    PRIMARY KEY (board_id, id)
);

-- The same local identity on every flat board, including empty snapshots.
INSERT INTO board_groups SELECT id, 'main', 'Items', 0, true FROM boards;
ALTER TABLE board_items
    ADD COLUMN group_id text COLLATE "C" NOT NULL DEFAULT 'main',
    DROP CONSTRAINT board_items_board_id_position_key,
    ADD UNIQUE (board_id, group_id, position),
    ADD FOREIGN KEY (board_id, group_id) REFERENCES board_groups(board_id, id);
