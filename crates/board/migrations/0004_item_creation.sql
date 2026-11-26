-- Snapshot/import rows have no interactive creator. Preserve the original
-- accepted spelling so replay after a rename cannot undo the rename.
ALTER TABLE board_items ADD COLUMN created_by uuid, ADD COLUMN creation_title text,
    ADD CHECK ((created_by IS NULL) = (creation_title IS NULL));
