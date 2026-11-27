-- Keep all earlier receipt shapes and identities intact on upgrade.
ALTER TABLE group_reorder_receipts
    DROP CONSTRAINT group_reorder_receipts_lifecycle_check,
    DROP CONSTRAINT group_reorder_receipts_check,
    ADD CHECK (lifecycle IN ('archive','restore','archive-selection')),
    ADD CHECK (CASE
      WHEN lifecycle='archive-selection' THEN
        group_id IS NULL AND num_nonnulls(item_id,item_revision,anchor_id,after_anchor)=0 AND
        num_nonnulls(selected_ids,selected_revisions,affected_ids)=3 AND
        cardinality(selected_ids) BETWEEN 1 AND 10000 AND
        cardinality(selected_ids)=cardinality(selected_revisions) AND
        cardinality(selected_ids)=cardinality(affected_ids)
      WHEN lifecycle IS NOT NULL THEN
        group_id IS NULL AND item_id IS NOT NULL AND item_revision IS NOT NULL AND
        num_nonnulls(anchor_id,after_anchor,selected_ids,selected_revisions,affected_ids)=0
      ELSE group_id IS NOT NULL AND (
        (num_nonnulls(item_id,anchor_id,after_anchor,item_revision)=4 AND num_nonnulls(selected_ids,selected_revisions,affected_ids)=0) OR
        (num_nonnulls(item_id,anchor_id,after_anchor,item_revision)=0 AND num_nonnulls(selected_ids,selected_revisions,affected_ids)=3)) END);
