CREATE TABLE workspaces (
    id uuid PRIMARY KEY,
    creator_id uuid NOT NULL REFERENCES accounts(id),
    name text NOT NULL,
    position integer NOT NULL,
    active boolean NOT NULL,
    is_open boolean NOT NULL
);
CREATE TABLE workspace_members (
    workspace_id uuid NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    account_id uuid NOT NULL REFERENCES accounts(id),
    PRIMARY KEY (workspace_id, account_id)
);

-- Keep the original classifications, creators and subscriber foreign keys.
-- A legacy policy has no catalog entry, but is still private in an open workspace.
ALTER TABLE open_workspace_private_boards RENAME TO board_policies;
ALTER TABLE private_board_subscribers RENAME TO board_subscribers;
ALTER TABLE board_policies
    ADD COLUMN workspace_id uuid REFERENCES workspaces(id),
    ADD COLUMN name text,
    ADD COLUMN position integer,
    ADD COLUMN active boolean NOT NULL DEFAULT true,
    ADD COLUMN kind text NOT NULL DEFAULT 'private' CHECK (kind IN ('main','private','shareable')),
    ADD CHECK ((workspace_id IS NULL AND name IS NULL AND position IS NULL AND kind = 'private')
            OR (workspace_id IS NOT NULL AND name IS NOT NULL AND position IS NOT NULL));
