ALTER TABLE board_groups ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0);
CREATE INDEX archived_groups_by_position ON board_groups(board_id, position, id)
    WHERE NOT active AND NOT deleted;
