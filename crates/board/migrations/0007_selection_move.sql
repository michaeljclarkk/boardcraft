-- One operation namespace across anchored reorders and unanchored batch moves.
ALTER TABLE group_reorder_receipts
    ALTER COLUMN item_id DROP NOT NULL, ALTER COLUMN anchor_id DROP NOT NULL,
    ALTER COLUMN after_anchor DROP NOT NULL, ALTER COLUMN item_revision DROP NOT NULL,
    ADD COLUMN selected_ids uuid[], ADD COLUMN selected_revisions bigint[],
    ADD COLUMN affected_ids uuid[],
    ADD CHECK ((num_nonnulls(item_id,anchor_id,after_anchor,item_revision) = 4
                AND num_nonnulls(selected_ids,selected_revisions,affected_ids) = 0)
            OR (num_nonnulls(item_id,anchor_id,after_anchor,item_revision) = 0
                AND num_nonnulls(selected_ids,selected_revisions,affected_ids) = 3));
