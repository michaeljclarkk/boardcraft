CREATE TABLE IF NOT EXISTS automations (
    id uuid PRIMARY KEY,
    board_id uuid NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name text NOT NULL,
    trigger text NOT NULL CHECK (trigger = 'item_renamed'),
    action text NOT NULL CHECK (action = 'record'),
    enabled boolean NOT NULL DEFAULT true,
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (btrim(name) <> '')
);
CREATE UNIQUE INDEX IF NOT EXISTS automations_board_name_key
    ON automations (board_id, lower(name));

CREATE TABLE IF NOT EXISTS automation_runs (
    id uuid PRIMARY KEY,
    automation_id uuid NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    board_id uuid NOT NULL,
    item_id uuid NOT NULL,
    item_title text NOT NULL,
    ran_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS automation_runs_recent
    ON automation_runs (automation_id, ran_at DESC);
