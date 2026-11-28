CREATE TABLE status_columns (
    board_id uuid PRIMARY KEY REFERENCES boards ON DELETE CASCADE,
    id uuid NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    title text NOT NULL DEFAULT 'Status',
    UNIQUE (board_id,id)
);
INSERT INTO status_columns(board_id) SELECT id FROM boards;

CREATE TABLE status_choices (
    key text PRIMARY KEY, label text NOT NULL, color text NOT NULL, position smallint NOT NULL UNIQUE
);
INSERT INTO status_choices VALUES ('working','Working on it','#579bfc',0),
    ('done','Done','#00c875',1), ('stuck','Stuck','#fdab3d',2);

ALTER TABLE board_items ADD UNIQUE(board_id,id);
CREATE TABLE status_cells (
    board_id uuid NOT NULL, column_id uuid NOT NULL, item_id uuid NOT NULL,
    value text REFERENCES status_choices(key), revision bigint NOT NULL CHECK (revision > 0),
    PRIMARY KEY (column_id,item_id),
    FOREIGN KEY (board_id,column_id) REFERENCES status_columns(board_id,id) ON DELETE CASCADE,
    FOREIGN KEY (board_id,item_id) REFERENCES board_items(board_id,id) ON DELETE CASCADE
);
