-- Label identity is local to its Status column. Existing cell keys and revisions
-- are preserved; null continues to mean the configured default label.
ALTER TABLE status_columns ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0);
CREATE TABLE status_labels (
    column_id uuid NOT NULL REFERENCES status_columns(id) ON DELETE CASCADE,
    key text NOT NULL,
    label text NOT NULL,
    color text NOT NULL,
    position smallint NOT NULL,
    PRIMARY KEY (column_id,key),
    UNIQUE (column_id,position)
);
INSERT INTO status_labels
    SELECT s.id,c.key,c.label,c.color,c.position FROM status_columns s CROSS JOIN status_choices c;
INSERT INTO status_labels SELECT id,'empty','','#c4c4c4',3 FROM status_columns;

ALTER TABLE status_cells DROP CONSTRAINT status_cells_value_fkey;
ALTER TABLE status_cells ADD FOREIGN KEY (column_id,value) REFERENCES status_labels(column_id,key);

CREATE FUNCTION seed_status_labels() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO status_labels SELECT NEW.id,key,label,color,position FROM status_choices;
    INSERT INTO status_labels VALUES (NEW.id,'empty','','#c4c4c4',3);
    RETURN NEW;
END $$;
CREATE TRIGGER status_column_labels AFTER INSERT ON status_columns
    FOR EACH ROW EXECUTE FUNCTION seed_status_labels();
