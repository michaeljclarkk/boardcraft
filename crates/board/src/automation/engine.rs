//! Evaluate saved typed recipes inside the caller's item transaction.
//!
//! The caller owns the item mutation and commits the item, runs and notification
//! intents together. The engine never keeps a snapshot between operations.
use super::model::*;
use super::rules;
use crate::{BoardId, ItemId};
use postgres::GenericClient;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

pub struct Event {
    pub kind: TriggerKind,
    pub column_id: Option<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub recipients: Vec<String>,
    pub actor: Uuid,
    source: Option<String>,
}

impl Event {
    pub fn item_created(actor: Uuid) -> Self {
        Self {
            kind: TriggerKind::ItemCreated,
            column_id: None,
            before: Vec::new(),
            after: Vec::new(),
            recipients: Vec::new(),
            actor,
            source: None,
        }
    }

    pub fn renamed(actor: Uuid, before: &str, after: &str) -> Self {
        Self {
            kind: TriggerKind::ColumnChanged,
            column_id: Some("Name".into()),
            before: vec![before.into()],
            after: vec![after.into()],
            recipients: Vec::new(),
            actor,
            source: None,
        }
    }

    pub fn column_changed(
        actor: Uuid,
        column: &str,
        before: Option<String>,
        after: Option<String>,
        source: Option<String>,
    ) -> Self {
        Self {
            kind: TriggerKind::ColumnChanged,
            column_id: Some(column.into()),
            before: before.into_iter().collect(),
            after: after.into_iter().collect(),
            recipients: Vec::new(),
            actor,
            source,
        }
    }

    /// The accepted event identity. It makes a retried enclosing write reuse the
    /// same run instead of applying the actions twice.
    fn idempotency(&self, item: ItemId, item_revision: i64, rule: &str) -> String {
        let source = self
            .source
            .clone()
            .unwrap_or_else(|| item_revision.to_string());
        format!("{}:{source}:{rule}", item.as_uuid().simple())
    }
}

#[derive(Debug)]
pub enum Error {
    Rejected(String),
    Database(postgres::Error),
}

impl From<postgres::Error> for Error {
    fn from(error: postgres::Error) -> Self {
        Self::Database(error)
    }
}

struct ItemState {
    id: String,
    title: String,
    group: String,
    revision: i64,
    created_by: Option<Uuid>,
    statuses: HashMap<String, String>,
}

fn load_item(
    db: &mut impl GenericClient,
    board: BoardId,
    item: ItemId,
) -> Result<ItemState, Error> {
    let row = db
        .query_opt(
            "SELECT i.title,i.group_id,i.revision,i.created_by FROM board_items i
             JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id)
             WHERE i.board_id=$1 AND i.id=$2 AND g.active AND i.state=0",
            &[&board.as_uuid(), &item.as_uuid()],
        )?
        .ok_or_else(|| Error::Rejected("The item is no longer available.".into()))?;
    let statuses = db
        .query(
            "SELECT c.column_id,c.value FROM status_cells c
             JOIN status_columns s ON s.id=c.column_id
             WHERE c.board_id=$1 AND c.item_id=$2 AND c.value IS NOT NULL",
            &[&board.as_uuid(), &item.as_uuid()],
        )?
        .into_iter()
        .map(|row| (row.get::<_, Uuid>(0).to_string(), row.get::<_, String>(1)))
        .collect();
    Ok(ItemState {
        id: item.as_uuid().to_string(),
        title: row.get(0),
        group: row.get(1),
        revision: row.get(2),
        created_by: row.get(3),
        statuses,
    })
}

fn item_values(item: &ItemState) -> HashMap<String, Vec<String>> {
    let mut values = HashMap::new();
    values.insert("Name".into(), vec![item.title.clone()]);
    values.insert("Group".into(), vec![item.group.clone()]);
    for (column, value) in &item.statuses {
        values.insert(column.clone(), vec![value.clone()]);
    }
    values
}

fn trigger_matches(trigger: &Trigger, event: &Event) -> bool {
    trigger.kind == event.kind
        && (trigger.kind != TriggerKind::ButtonClicked || trigger.column_id == event.column_id)
        && (trigger.kind != TriggerKind::PersonAssigned
            || trigger.column_id.as_deref().unwrap_or("Assigned")
                == event.column_id.as_deref().unwrap_or(""))
        && (trigger.kind != TriggerKind::ColumnChanged
            || (trigger
                .column_id
                .as_deref()
                .is_some_and(|column| event.column_id.as_deref() == Some(column))
                && (trigger.value.is_none()
                    || trigger.value.as_deref() == Some("") && event.after.is_empty()
                    || event
                        .after
                        .iter()
                        .any(|value| Some(value) == trigger.value.as_ref()))
                && (trigger.previous_value.is_none()
                    || trigger.previous_value.as_deref() == Some("") && event.before.is_empty()
                    || event
                        .before
                        .iter()
                        .any(|value| Some(value) == trigger.previous_value.as_ref()))))
}

/// Run every ready rule for this event, including the events produced by their
/// actions. Each rule runs at most once per accepted item transition.
pub fn apply(
    db: &mut impl GenericClient,
    board: BoardId,
    item: ItemId,
    event: Event,
) -> Result<usize, Error> {
    let rules = load_rules(db, board)?;
    if rules.is_empty() {
        return Ok(0);
    }
    let catalog = super::store::catalog(db, board.as_uuid())
        .map_err(|_| Error::Rejected("The automation catalog could not be read.".to_owned()))?;
    let mut state = load_item(db, board, item)?;
    let mut queue = VecDeque::from([event]);
    let mut evaluated = HashSet::new();
    let mut runs = 0;
    while let Some(event) = queue.pop_front() {
        let values = item_values(&state);
        let item_revision = state.revision;
        let mut before = values.clone();
        if event.kind == TriggerKind::ItemCreated {
            // A created item had no cells: every populated column is a change.
            before.clear();
        }
        for rule in &rules {
            let Some(definition) = &rule.definition else {
                continue;
            };
            if evaluated.contains(&rule.id) || !trigger_matches(&definition.trigger, &event) {
                continue;
            }
            evaluated.insert(rule.id.clone());
            let condition = rules::conditions_match_all(
                &definition.conditions,
                definition.conditions_mode,
                &values,
            );
            let branches: Vec<bool> = definition
                .action_groups
                .iter()
                .map(|group| rules::conditions_match_all(&group.conditions, group.mode, &values))
                .collect();
            let outcome = if !rule.enabled {
                Outcome::SkippedDisabled
            } else if !condition {
                Outcome::ConditionNotMet
            } else {
                Outcome::Succeeded
            };
            let idempotency = event.idempotency(item, item_revision, &rule.id);
            let trigger_snapshot = serde_json::json!({
                "trigger": {
                    "kind": event.kind,
                    "columnId": event.column_id,
                    "before": event.before,
                    "after": event.after,
                },
                "rule": {
                    "trigger": rule.trigger,
                    "conditions": rule.conditions,
                    "enabled": rule.enabled,
                    "disposition": rule.disposition,
                    "evidence": rule.evidence,
                    "decision": rule.decision,
                    "definition": definition,
                }
            })
            .to_string();
            let inserted = db.query_opt(
                "INSERT INTO automation_runs(
                        id,automation_id,board_id,item_id,item_title,idempotency_key,
                        rule_name,rule_revision,item_revision,actor_id,outcome,
                        trigger_snapshot,action_snapshot)
                     VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,$6,$7::bigint,$8,$9,$10,$11,'{}')
                     ON CONFLICT (idempotency_key) DO NOTHING RETURNING id",
                &[
                    &store_uuid(&rule.id)?,
                    &board.as_uuid(),
                    &item.as_uuid(),
                    &state.title,
                    &idempotency,
                    &rule.name,
                    &rule.revision,
                    &item_revision,
                    &event.actor,
                    &outcome_name(outcome),
                    &trigger_snapshot,
                ],
            )?;
            if inserted.is_none() {
                continue;
            }
            runs += 1;
            let mut index = 0usize;
            if outcome == Outcome::Succeeded {
                for action in &definition.actions {
                    execute(
                        db, board, item, &mut state, action, index, &event, rule, &catalog,
                    )?;
                    index += 1;
                }
                for (branch, group) in branches.iter().zip(&definition.action_groups) {
                    for action in &group.actions {
                        if *branch {
                            execute(
                                db, board, item, &mut state, action, index, &event, rule, &catalog,
                            )?;
                        }
                        index += 1;
                    }
                }
            }
            let action_snapshot = serde_json::json!({
                "actions": rule.actions,
                "assigneeIds": rule.assignee_ids,
                "groupId": state.group,
                "statuses": state.statuses,
                "configuredActions": definition.actions,
                "configuredActionGroups": definition.action_groups,
                "matchedActionGroups": branches,
                "executed": outcome == Outcome::Succeeded,
            })
            .to_string();
            db.execute(
                "UPDATE automation_runs SET action_snapshot=$2 WHERE idempotency_key=$1",
                &[&idempotency, &action_snapshot],
            )?;
        }
        enqueue_changes(&mut queue, &before, &item_values(&state), &event);
    }
    Ok(runs)
}

fn store_uuid(rule: &str) -> Result<Uuid, Error> {
    super::store::parse_rule_id(rule)
        .ok_or_else(|| Error::Rejected("This automation identity is not valid.".into()))
}

fn outcome_name(outcome: Outcome) -> String {
    match outcome {
        Outcome::Succeeded => "succeeded".into(),
        Outcome::SkippedDisabled => "skipped_disabled".into(),
        Outcome::ConditionNotMet => "condition_not_met".into(),
    }
}

fn load_rules(db: &mut impl GenericClient, board: BoardId) -> Result<Vec<Rule>, Error> {
    let rows = db.query(
        "SELECT id,name,trigger,conditions,actions,enabled,revision::bigint,disposition,
                dependency_issue,product_column_id,assignee_ids,source_recipes,
                evidence,decision,canonical_rule_id,definition::text,position::bigint,archived,
                created_by,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
         FROM automations WHERE board_id=$1 AND NOT archived AND disposition='ready'
           AND dependency_issue IS NULL ORDER BY position,id LIMIT 251",
        &[&board.as_uuid()],
    )?;
    if rows.len() > rules::MAXIMUM_RULES as usize {
        return Err(Error::Rejected(
            "The automation configuration limit was exceeded. Ask the Owner to archive unused rules."
                .into(),
        ));
    }
    rows.into_iter()
        .map(|row| {
            let definition: Option<String> = row.try_get(15)?;
            Ok(Rule {
                id: super::store::rule_id(row.get::<_, Uuid>(0)),
                name: row.get(1),
                trigger: row.get(2),
                conditions: row.get(3),
                actions: row.get(4),
                enabled: row.get(5),
                revision: row.get(6),
                disposition: Disposition::Ready,
                dependency_issue: row.get(8),
                product_column_id: row.get(9),
                assignee_ids: row.get(10),
                source_recipes: row.get(11),
                evidence: row.get(12),
                decision: row.get(13),
                canonical_rule_id: row.get(14),
                definition: definition.and_then(|text| serde_json::from_str(&text).ok()),
                position: row.get(16),
                archived: row.get(17),
                created_by: row.try_get::<_, Uuid>(18)?.to_string(),
                created_at: row.get(19),
            })
        })
        .collect::<Result<Vec<_>, postgres::Error>>()
        .map_err(Error::Database)
}

#[allow(clippy::too_many_arguments)]
fn execute(
    db: &mut impl GenericClient,
    board: BoardId,
    item: ItemId,
    state: &mut ItemState,
    action: &Action,
    index: usize,
    event: &Event,
    rule: &Rule,
    catalog: &Catalog,
) -> Result<(), Error> {
    match action.kind {
        ActionKind::ChangeStatus => {
            let column = action.column_id.as_deref().unwrap_or("");
            let id = Uuid::parse_str(column).map_err(|_| {
                Error::Rejected(format!(
                    "Automation '{}' has an invalid status column.",
                    rule.name
                ))
            })?;
            let value = action.value.as_deref().unwrap_or("");
            let known = db.query_opt("SELECT 1 FROM status_choices WHERE key=$1", &[&value])?;
            if known.is_none() {
                return Err(Error::Rejected(format!(
                    "Automation '{}' cannot set an unavailable status. Update the recipe, then retry.",
                    rule.name
                )));
            }
            db.execute(
                "INSERT INTO status_cells(board_id,column_id,item_id,value,revision)
                 VALUES($1,$2,$3,$4,1)
                 ON CONFLICT(column_id,item_id) DO UPDATE
                 SET value=excluded.value,revision=status_cells.revision+1",
                &[&board.as_uuid(), &id, &item.as_uuid(), &value],
            )?;
            state.statuses.insert(id.to_string(), value.to_owned());
        }
        ActionKind::MoveToGroup => {
            let group = action.value.as_deref().unwrap_or("");
            db.query_opt(
                "SELECT 1 FROM board_groups WHERE board_id=$1 AND id=$2 AND active",
                &[&board.as_uuid(), &group],
            )?
            .ok_or_else(|| {
                Error::Rejected(format!(
                    "Automation '{}' cannot move the item to an unavailable group.",
                    rule.name
                ))
            })?;
            db.execute(
                "UPDATE board_items SET group_id=$3,revision=revision+1,group_revision=revision+1
                 WHERE board_id=$1 AND id=$2",
                &[&board.as_uuid(), &item.as_uuid(), &group],
            )?;
            state.group = group.to_owned();
            state.revision += 1;
        }
        ActionKind::Notify => {
            let recipients = match action.recipient() {
                Recipient::Creator => vec![state.created_by.unwrap_or(event.actor).to_string()],
                Recipient::People => action.person_ids.clone(),
                Recipient::TriggerPeople => event.recipients.clone(),
                Recipient::Assigned => Vec::new(),
            };
            let title = action.title.as_deref().unwrap_or("Automation notification");
            let message = action.message.as_deref().unwrap_or("");
            let mut seen = HashSet::new();
            for recipient in recipients {
                if !seen.insert(recipient.clone()) {
                    continue;
                }
                let Ok(recipient) = Uuid::parse_str(&recipient) else {
                    continue;
                };
                let exists = db.query_opt(
                    "SELECT 1 FROM accounts WHERE id=$1 AND active",
                    &[&recipient],
                )?;
                if exists.is_none() {
                    continue;
                }
                db.execute(
                    "INSERT INTO inbox_notifications(
                        id,board_id,item_id,automation_id,recipient_id,kind,title,message)
                     VALUES(gen_random_uuid(),$1,$2,$3,$4,'automation',$5,$6)
                     ON CONFLICT DO NOTHING",
                    &[
                        &board.as_uuid(),
                        &item.as_uuid(),
                        &store_uuid(&rule.id)?,
                        &recipient,
                        &expand(title, state, event, catalog),
                        &expand(message, state, event, catalog),
                    ],
                )?;
            }
            let _ = index;
        }
        ActionKind::AssignPeople
        | ActionKind::ReplacePeople
        | ActionKind::ClearPeople
        | ActionKind::AssignCreator
        | ActionKind::SetDate
        | ActionKind::PushDate => {
            return Err(Error::Rejected(format!(
                "Automation '{}' needs a column this board does not have. Update the recipe, then retry.",
                rule.name
            )));
        }
    }
    Ok(())
}

fn expand(message: &str, state: &ItemState, event: &Event, catalog: &Catalog) -> String {
    let actor = catalog
        .people
        .iter()
        .find(|person| person.id == event.actor.to_string())
        .map(|person| person.name.as_str())
        .unwrap_or("");
    message
        .replace("{item.name}", &state.title)
        .replace("{board.name}", &catalog.board_name)
        .replace("{item.id}", &state.id)
        .replace("{post.id}", "")
        .replace("{post.url}", "")
        .replace("{actor.id}", &event.actor.to_string())
        .replace("{actor.name}", actor)
        .replace("{reaction.type}", "")
}

fn enqueue_changes(
    queue: &mut VecDeque<Event>,
    before: &HashMap<String, Vec<String>>,
    after: &HashMap<String, Vec<String>>,
    source: &Event,
) {
    for (column, value) in after {
        let previous = before.get(column).cloned().unwrap_or_default();
        if &previous != value {
            queue.push_back(Event {
                kind: TriggerKind::ColumnChanged,
                column_id: Some(column.clone()),
                before: previous,
                after: value.clone(),
                recipients: Vec::new(),
                actor: source.actor,
                source: source.source.clone(),
            });
        }
    }
}
