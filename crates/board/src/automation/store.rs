//! PostgreSQL ownership of automation rules, the board catalog and run history.
use super::model::*;
use super::rules::{self, Invalid};
use postgres::GenericClient;
use uuid::Uuid;

pub const RULE_PAGE: usize = 250;
pub const HISTORY_PAGE: usize = 50;

#[derive(Debug)]
pub enum StoreError {
    Invalid(&'static str),
    Conflict,
    Missing,
    NotReady,
    Database(postgres::Error),
}

impl From<Invalid> for StoreError {
    fn from(error: Invalid) -> Self {
        match error {
            Invalid::Message(message) => Self::Invalid(message),
        }
    }
}

pub fn parse_rule_id(id: &str) -> Option<Uuid> {
    Uuid::parse_str(id.strip_prefix("custom-").unwrap_or(id)).ok()
}

pub fn rule_id(id: Uuid) -> String {
    format!("custom-{}", id.simple())
}

pub fn catalog(db: &mut impl GenericClient, board: Uuid) -> Result<Catalog, StoreError> {
    let board_name = String::new();
    let status = db
        .query_opt(
            "SELECT id,title FROM status_columns WHERE board_id=$1",
            &[&board],
        )
        .map_err(StoreError::Database)?;
    let mut columns = vec![Column {
        id: "Name".into(),
        name: "Name".into(),
        kind: "text".into(),
        choices: Vec::new(),
    }];
    if let Some(status) = status {
        let id: Uuid = status.get(0);
        let title: String = status.get(1);
        let choices = db
            .query(
                "SELECT key,label FROM status_choices ORDER BY position",
                &[],
            )
            .map_err(StoreError::Database)?
            .into_iter()
            .map(|row| Choice {
                value: row.get(0),
                name: row.get(1),
            })
            .collect();
        columns.push(Column {
            id: id.to_string(),
            name: title,
            kind: "status".into(),
            choices,
        });
    }
    let groups = db
        .query(
            "SELECT id,name FROM board_groups WHERE board_id=$1 AND active ORDER BY position,id",
            &[&board],
        )
        .map_err(StoreError::Database)?
        .into_iter()
        .map(|row| Choice {
            value: row.get(0),
            name: row.get(1),
        })
        .collect();
    columns.push(Column {
        id: "Group".into(),
        name: "Group".into(),
        kind: "group".into(),
        choices: groups,
    });
    let people = db
        .query(
            "SELECT id,username,role FROM accounts WHERE active ORDER BY lower(username),id",
            &[],
        )
        .map_err(StoreError::Database)?
        .into_iter()
        .map(|row| {
            let role: String = row.get(2);
            Person {
                id: row.get::<_, Uuid>(0).to_string(),
                name: row.get(1),
                can_assign: role != "viewer",
                can_mention: true,
                team: false,
            }
        })
        .collect();
    let items = db
        .query(
            "SELECT i.id,i.title FROM board_items i
             JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id)
             WHERE i.board_id=$1 AND g.active AND i.state=0
             ORDER BY g.position,g.id,i.position,i.id LIMIT 500",
            &[&board],
        )
        .map_err(StoreError::Database)?
        .into_iter()
        .map(|row| ItemRef {
            id: row.get::<_, Uuid>(0).to_string(),
            name: row.get(1),
        })
        .collect();
    Ok(Catalog {
        board_name,
        columns,
        triggers: TriggerKind::ALL.to_vec(),
        actions: ActionKind::ALL.to_vec(),
        maximum_conditions: rules::MAXIMUM_CONDITIONS_PER_GROUP as i64,
        maximum_actions: rules::MAXIMUM_ACTIONS as i64,
        maximum_rules: rules::MAXIMUM_RULES,
        people,
        items,
    })
}

pub fn list(
    db: &mut impl GenericClient,
    board: Uuid,
    archived: bool,
    offset: i64,
) -> Result<Vec<Rule>, StoreError> {
    if offset < 0 {
        return Err(StoreError::Invalid("Reload the automation rules."));
    }
    let rows = db
        .query(
            "SELECT id,name,trigger,conditions,actions,enabled,revision::bigint,disposition,
                    dependency_issue,product_column_id,assignee_ids,source_recipes,
                    evidence,decision,canonical_rule_id,definition::text,position::bigint,archived,
                    created_by,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
             FROM automations WHERE board_id=$1 AND archived=$2
             ORDER BY position,id LIMIT $3 OFFSET $4",
            &[&board, &archived, &(RULE_PAGE as i64), &offset],
        )
        .map_err(StoreError::Database)?;
    rows.into_iter().map(read_rule).collect()
}

fn read_rule(row: postgres::Row) -> Result<Rule, StoreError> {
    let definition: Option<String> = row.try_get(15).map_err(StoreError::Database)?;
    let definition = definition
        .map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|_| StoreError::Invalid("Reload the automation rules."))?;
    Ok(Rule {
        id: rule_id(row.get::<_, Uuid>(0)),
        name: row.get(1),
        trigger: row.get(2),
        conditions: row.get(3),
        actions: row.get(4),
        enabled: row.get(5),
        revision: row.get(6),
        disposition: match row.get::<_, String>(7).as_str() {
            "review_required" => Disposition::ReviewRequired,
            "obsolete" => Disposition::Obsolete,
            _ => Disposition::Ready,
        },
        dependency_issue: row.get(8),
        product_column_id: row.get(9),
        assignee_ids: row.get(10),
        source_recipes: row.get(11),
        evidence: row.get(12),
        decision: row.get(13),
        canonical_rule_id: row.get(14),
        definition,
        position: row.get(16),
        archived: row.get(17),
        created_by: row
            .try_get::<_, Uuid>(18)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        created_at: row.get(19),
    })
}

fn load_rule(db: &mut impl GenericClient, board: Uuid, id: Uuid) -> Result<Rule, StoreError> {
    db.query_opt(
        "SELECT id,name,trigger,conditions,actions,enabled,revision::bigint,disposition,
                dependency_issue,product_column_id,assignee_ids,source_recipes,
                evidence,decision,canonical_rule_id,definition::text,position::bigint,archived,
                created_by,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
         FROM automations WHERE board_id=$1 AND id=$2",
        &[&board, &id],
    )
    .map_err(StoreError::Database)?
    .map(read_rule)
    .transpose()?
    .ok_or(StoreError::Missing)
}

pub fn save(
    db: &mut impl GenericClient,
    board: Uuid,
    input: &SaveInput,
    actor: Uuid,
) -> Result<Rule, StoreError> {
    let name = rules::name(&input.name)?;
    let id = parse_rule_id(&input.id).ok_or(StoreError::Invalid(
        "A new automation needs a stable creation identity.",
    ))?;
    let catalog = catalog(db, board)?;
    rules::validate(&input.definition, &catalog)?;
    let sentences = rules::sentences(&input.definition, &catalog);
    let product_column_id = input
        .definition
        .trigger
        .column_id
        .as_ref()
        .filter(|id| {
            catalog
                .column(id)
                .is_some_and(|column| column.kind == "product")
        })
        .cloned();
    let assignee_ids = rules::assignee_ids(&input.definition);
    let definition = serde_json::to_string(&input.definition)
        .map_err(|_| StoreError::Invalid("The automation recipe could not be saved."))?;
    if input.expected_revision == 0 {
        db.execute("SELECT pg_advisory_xact_lock(729151)", &[])
            .map_err(StoreError::Database)?;
        if let Some(existing) = db
            .query_opt(
                "SELECT created_by,board_id FROM automations WHERE id=$1",
                &[&id],
            )
            .map_err(StoreError::Database)?
        {
            let creator: Uuid = existing.get(0);
            let existing_board: Uuid = existing.get(1);
            if creator != actor || existing_board != board {
                return Err(StoreError::Conflict);
            }
            return load_rule(db, board, id);
        }
        let active: i64 = db
            .query_one(
                "SELECT count(*) FROM automations WHERE board_id=$1 AND NOT archived",
                &[&board],
            )
            .map_err(StoreError::Database)?
            .get(0);
        if active >= rules::MAXIMUM_RULES {
            return Err(StoreError::Invalid(
                "Archive an automation before adding another. This board supports up to 250 active configurations.",
            ));
        }
        db.execute(
            "INSERT INTO automations(
                id,board_id,name,trigger,action,enabled,created_by,definition,conditions,
                actions,revision,disposition,dependency_issue,product_column_id,
                assignee_ids,source_recipes,evidence,decision,canonical_rule_id,position,archived)
             VALUES($1,$2,$3,$4,'configured',$5,$6,$7::text::jsonb,$8,$9,1,'ready',NULL,$10,$11,$12,$13,$14,NULL,1000,false)",
            &[
                &id,
                &board,
                &name,
                &sentences.trigger,
                &input.enabled,
                &actor,
                &definition,
                &sentences.conditions,
                &sentences.actions,
                &product_column_id,
                &assignee_ids,
                &Vec::<String>::new(),
                &Vec::<String>::new(),
                &"Configured locally by the Owner. Changes apply to future events; accepted runs and deliveries retain their snapshots.",
            ],
        )
        .map_err(StoreError::Database)?;
        load_rule(db, board, id)
    } else {
        let current = load_rule(db, board, id)?;
        if current.revision != input.expected_revision {
            return Err(StoreError::Conflict);
        }
        if current.disposition != Disposition::Ready || current.archived {
            return Err(StoreError::Invalid(
                "Resolve this recipe or restore it before editing.",
            ));
        }
        db.execute(
            "UPDATE automations SET name=$3,trigger=$4,enabled=$5,definition=$6::text::jsonb,
                conditions=$7,actions=$8,revision=revision+1,dependency_issue=NULL,
                product_column_id=$9,assignee_ids=$10
             WHERE board_id=$1 AND id=$2",
            &[
                &board,
                &id,
                &name,
                &sentences.trigger,
                &input.enabled,
                &definition,
                &sentences.conditions,
                &sentences.actions,
                &product_column_id,
                &assignee_ids,
            ],
        )
        .map_err(StoreError::Database)?;
        load_rule(db, board, id)
    }
}

pub fn set_enabled(
    db: &mut impl GenericClient,
    board: Uuid,
    id: Uuid,
    expected_revision: i64,
    enabled: bool,
    actor: Uuid,
) -> Result<Rule, StoreError> {
    let rule = load_rule(db, board, id)?;
    if rule.revision != expected_revision {
        return Err(StoreError::Conflict);
    }
    if enabled {
        if rule.disposition != Disposition::Ready || rule.archived {
            return Err(StoreError::NotReady);
        }
        let definition = rule.definition.as_ref().ok_or(StoreError::NotReady)?;
        rules::validate(definition, &catalog(db, board)?)?;
    }
    if rule.enabled == enabled {
        return Ok(rule);
    }
    db.execute(
        "UPDATE automations SET enabled=$3,revision=revision+1,dependency_issue=NULL
         WHERE board_id=$1 AND id=$2",
        &[&board, &id, &enabled],
    )
    .map_err(StoreError::Database)?;
    let _ = actor;
    load_rule(db, board, id)
}

pub fn set_archived(
    db: &mut impl GenericClient,
    board: Uuid,
    id: Uuid,
    expected_revision: i64,
    archived: bool,
) -> Result<Rule, StoreError> {
    db.execute("SELECT pg_advisory_xact_lock(729151)", &[])
        .map_err(StoreError::Database)?;
    let rule = load_rule(db, board, id)?;
    if rule.revision != expected_revision {
        return Err(StoreError::Conflict);
    }
    if rule.archived == archived {
        return Ok(rule);
    }
    if !archived {
        let active: i64 = db
            .query_one(
                "SELECT count(*) FROM automations WHERE board_id=$1 AND NOT archived",
                &[&board],
            )
            .map_err(StoreError::Database)?
            .get(0);
        if active >= rules::MAXIMUM_RULES {
            return Err(StoreError::Invalid(
                "Archive another automation before restoring this one.",
            ));
        }
    }
    db.execute(
        "UPDATE automations SET archived=$3,enabled=false,revision=revision+1
         WHERE board_id=$1 AND id=$2",
        &[&board, &id, &archived],
    )
    .map_err(StoreError::Database)?;
    load_rule(db, board, id)
}

pub fn history(
    db: &mut impl GenericClient,
    board: Uuid,
    rule: Option<Uuid>,
    offset: i64,
) -> Result<History, StoreError> {
    if offset < 0 {
        return Err(StoreError::Invalid("Reload the run history."));
    }
    let rows = db
        .query(
            "SELECT id,idempotency_key,automation_id,rule_name,rule_revision::bigint,item_id,
                    item_revision,actor_id,outcome,trigger_snapshot,action_snapshot,
                    to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
             FROM automation_runs
             WHERE board_id=$1 AND ($2::uuid IS NULL OR automation_id=$2)
             ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4",
            &[&board, &rule, &((HISTORY_PAGE + 1) as i64), &offset],
        )
        .map_err(StoreError::Database)?;
    let has_more = rows.len() > HISTORY_PAGE;
    let runs = rows
        .into_iter()
        .take(HISTORY_PAGE)
        .map(|row| {
            Ok(Run {
                id: row.get::<_, Uuid>(0).to_string(),
                idempotency_key: row.get(1),
                rule_id: rule_id(row.get::<_, Uuid>(2)),
                rule_name: row.get(3),
                rule_revision: row.get(4),
                item_id: row.get::<_, Uuid>(5).to_string(),
                item_revision: row.get(6),
                actor_id: row.get::<_, Uuid>(7).to_string(),
                outcome: match row.get::<_, String>(8).as_str() {
                    "skipped_disabled" => Outcome::SkippedDisabled,
                    "condition_not_met" => Outcome::ConditionNotMet,
                    _ => Outcome::Succeeded,
                },
                trigger_snapshot: row.get(9),
                action_snapshot: row.get(10),
                created_at: row.get(11),
            })
        })
        .collect::<Result<Vec<_>, postgres::Error>>()
        .map_err(StoreError::Database)?;
    Ok(History { runs, has_more })
}
