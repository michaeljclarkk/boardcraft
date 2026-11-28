ALTER TABLE saved_views
    ADD COLUMN status_values text[] NOT NULL DEFAULT '{}',
    ADD COLUMN include_empty_status boolean NOT NULL DEFAULT false,
    ADD COLUMN group_ids text[] NOT NULL DEFAULT '{}';

-- A personal name belongs to its owner; another user's private view must not
-- reserve that name or disclose its existence through a conflict response.
DROP INDEX saved_views_board_name_key;
CREATE UNIQUE INDEX saved_views_personal_name_key
    ON saved_views(board_id, created_by, lower(name)) WHERE NOT shared;
CREATE UNIQUE INDEX saved_views_shared_name_key
    ON saved_views(board_id, lower(name)) WHERE shared;
