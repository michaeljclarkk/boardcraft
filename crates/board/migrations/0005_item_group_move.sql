-- v4 has no creation times. At equal positions put undated rows first,
-- ordered by UUID; do not assign migration time as their creation time.
ALTER TABLE board_items
    DROP CONSTRAINT board_items_board_id_group_id_position_key,
    ADD COLUMN created_at timestamptz,
    ADD COLUMN group_revision bigint NOT NULL DEFAULT 1 CHECK (group_revision BETWEEN 1 AND revision),
    ADD COLUMN creation_group_id text;
ALTER TABLE board_items ALTER COLUMN created_at SET DEFAULT clock_timestamp();
UPDATE board_items SET creation_group_id = group_id WHERE created_by IS NOT NULL;
CREATE INDEX board_items_group_order ON board_items
    (board_id, group_id, position, created_at NULLS FIRST, id);
