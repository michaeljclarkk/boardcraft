CREATE TABLE IF NOT EXISTS saved_views (
    id uuid PRIMARY KEY,
    board_id uuid NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name text NOT NULL,
    filter text NOT NULL,
    created_by uuid NOT NULL,
    shared boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (btrim(name) <> '')
);
CREATE UNIQUE INDEX IF NOT EXISTS saved_views_board_name_key
    ON saved_views (board_id, lower(name));
