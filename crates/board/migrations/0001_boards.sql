CREATE TABLE boards (
    id uuid PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000')
);

CREATE TABLE board_items (
    id uuid PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    board_id uuid NOT NULL REFERENCES boards(id),
    position bigint NOT NULL CHECK (position >= 0),
    title text NOT NULL,
    UNIQUE (board_id, position)
);
