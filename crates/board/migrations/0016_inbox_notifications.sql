CREATE TABLE IF NOT EXISTS inbox_notifications (
    id uuid PRIMARY KEY,
    board_id uuid NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    item_id uuid NOT NULL,
    automation_id uuid REFERENCES automations(id) ON DELETE SET NULL,
    recipient_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind = 'automation'),
    title text NOT NULL,
    message text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    read_at timestamptz,
    seen_at timestamptz,
    deleted_at timestamptz,
    CHECK (btrim(title) <> '')
);
CREATE INDEX IF NOT EXISTS inbox_notifications_recipient_recent
    ON inbox_notifications (recipient_id, created_at DESC);
