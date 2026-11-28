CREATE TABLE item_activity (
    board_id uuid NOT NULL,
    item_id uuid NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    actor_id uuid NOT NULL,
    actor_name text NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    field text NOT NULL CHECK (field = 'name'),
    before_value text NOT NULL,
    after_value text NOT NULL CHECK (after_value <> before_value),
    PRIMARY KEY (board_id, item_id, revision)
);

CREATE FUNCTION reject_item_activity_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Item activity is append-only';
END;
$$;
CREATE TRIGGER immutable_item_activity BEFORE UPDATE OR DELETE ON item_activity
    FOR EACH ROW EXECUTE FUNCTION reject_item_activity_mutation();
