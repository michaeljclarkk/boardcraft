ALTER TABLE board_groups ADD COLUMN colour text NOT NULL DEFAULT '#579bfc'
    CHECK (colour ~ '^#[0-9a-fA-F]{6}$');
