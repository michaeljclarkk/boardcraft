-- The typed automation engine: saved definitions, revisions, archive state and
-- accepted runs. The first slice's fixed rename rule is migrated to the same
-- closed definition shape instead of being discarded.
ALTER TABLE automations
    DROP CONSTRAINT IF EXISTS automations_trigger_check,
    DROP CONSTRAINT IF EXISTS automations_action_check,
    ADD COLUMN IF NOT EXISTS definition jsonb,
    ADD COLUMN IF NOT EXISTS conditions text[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS actions text[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS revision integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS disposition text NOT NULL DEFAULT 'ready',
    ADD COLUMN IF NOT EXISTS dependency_issue text,
    ADD COLUMN IF NOT EXISTS product_column_id text,
    ADD COLUMN IF NOT EXISTS assignee_ids text[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS source_recipes text[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS evidence text[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS decision text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS canonical_rule_id text,
    ADD COLUMN IF NOT EXISTS position integer NOT NULL DEFAULT 1000,
    ADD COLUMN IF NOT EXISTS archived boolean NOT NULL DEFAULT false;
DROP INDEX IF EXISTS automations_board_name_key;
CREATE INDEX IF NOT EXISTS automations_board_order
    ON automations (board_id, archived, position, id);

UPDATE automations
SET definition = jsonb_build_object(
        'trigger', jsonb_build_object('kind', 'column_changed', 'column_id', 'Name'),
        'conditions', '[]'::jsonb,
        'actions', jsonb_build_array(
            jsonb_build_object(
                'kind', 'notify',
                'recipient', 'people',
                'person_ids', jsonb_build_array(created_by::text),
                'title', 'Automation notification',
                'message', 'Item {item.name} was renamed.'
            )
        ),
        'conditions_mode', 'all',
        'action_groups', '[]'::jsonb
    ),
    trigger = 'When Name changes',
    conditions = '{}',
    actions = ARRAY['Notify Creator: Item {item.name} was renamed.'],
    decision = 'Imported from the first rename-recording rule.'
WHERE definition IS NULL;

ALTER TABLE automations
    ALTER COLUMN definition SET NOT NULL,
    ADD CONSTRAINT automations_disposition_check
        CHECK (disposition IN ('ready', 'review_required', 'obsolete'));

ALTER TABLE automation_runs
    ADD COLUMN IF NOT EXISTS idempotency_key text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS rule_name text NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS rule_revision integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS item_revision bigint NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS actor_id uuid,
    ADD COLUMN IF NOT EXISTS outcome text NOT NULL DEFAULT 'succeeded',
    ADD COLUMN IF NOT EXISTS trigger_snapshot text NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS action_snapshot text NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT now();

UPDATE automation_runs r
SET idempotency_key = 'legacy:' || r.id,
    rule_name = coalesce(a.name, ''),
    actor_id = a.created_by,
    created_at = r.ran_at
FROM automations a
WHERE a.id = r.automation_id;

ALTER TABLE automation_runs ALTER COLUMN actor_id SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS automation_runs_idempotency_key
    ON automation_runs (idempotency_key);
CREATE INDEX IF NOT EXISTS automation_runs_board_recent
    ON automation_runs (board_id, created_at DESC, id DESC);
ALTER TABLE automation_runs
    ADD CONSTRAINT automation_runs_outcome_check
        CHECK (outcome IN ('succeeded', 'skipped_disabled', 'condition_not_met'));
