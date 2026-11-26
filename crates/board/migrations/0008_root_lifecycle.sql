-- Existing items have Active own state even when their group is archived.
ALTER TABLE board_items ADD COLUMN state integer NOT NULL DEFAULT 0 CHECK (state BETWEEN 0 AND 3);
ALTER TABLE board_groups ADD COLUMN deleted boolean NOT NULL DEFAULT false CHECK (NOT deleted OR NOT active);

ALTER TABLE group_reorder_receipts DROP CONSTRAINT group_reorder_receipts_check,
    ALTER COLUMN group_id DROP NOT NULL,
    ADD COLUMN lifecycle text CHECK (lifecycle IN ('archive','restore')),
    ADD CHECK (CASE WHEN lifecycle IS NOT NULL THEN
        group_id IS NULL AND item_id IS NOT NULL AND item_revision IS NOT NULL AND
        num_nonnulls(anchor_id,after_anchor,selected_ids,selected_revisions,affected_ids)=0
      ELSE group_id IS NOT NULL AND (
        (num_nonnulls(item_id,anchor_id,after_anchor,item_revision)=4 AND num_nonnulls(selected_ids,selected_revisions,affected_ids)=0) OR
        (num_nonnulls(item_id,anchor_id,after_anchor,item_revision)=0 AND num_nonnulls(selected_ids,selected_revisions,affected_ids)=3)) END);
