ALTER TABLE boards ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0);
ALTER TABLE board_items ADD COLUMN position_revision bigint NOT NULL DEFAULT 1
    CHECK (position_revision BETWEEN 1 AND revision);

-- Baselines identify the exact accepted command (and hence its acknowledgment).
CREATE TABLE group_reorder_receipts (
    operation_id uuid PRIMARY KEY,
    actor_id uuid NOT NULL, board_id uuid NOT NULL, item_id uuid NOT NULL,
    group_id text NOT NULL, anchor_id uuid NOT NULL, after_anchor boolean NOT NULL,
    board_revision bigint NOT NULL, item_revision bigint NOT NULL
);
