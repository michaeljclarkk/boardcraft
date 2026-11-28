//! Validation and human sentences for the same closed definition the editor
//! and executor read.
use super::model::*;
use std::collections::{HashMap, HashSet};

pub const VALUE_CHARACTERS: usize = 240;
pub const MAXIMUM_OFFSET_DAYS: i32 = 3650;
pub const NAME_CHARACTERS: usize = 160;
pub const TITLE_CHARACTERS: usize = 160;
pub const MESSAGE_CHARACTERS: usize = 4000;
pub const MAXIMUM_ACTION_GROUPS: usize = 8;
pub const MAXIMUM_ACTIONS: usize = 8;
pub const MAXIMUM_CONDITIONS_PER_GROUP: usize = 8;
pub const MAXIMUM_CONDITIONS: usize = 30;
pub const MAXIMUM_RULES: i64 = 250;

pub enum Invalid {
    Message(&'static str),
}

type Valid = Result<(), Invalid>;

fn require(valid: bool, message: &'static str) -> Valid {
    if valid {
        Ok(())
    } else {
        Err(Invalid::Message(message))
    }
}

pub fn name(raw: &str) -> Result<&str, Invalid> {
    let name = raw.trim();
    require(
        !name.is_empty() && name.encode_utf16().count() <= NAME_CHARACTERS,
        "Name this automation using 1–160 characters.",
    )?;
    Ok(name)
}

fn column<'a>(catalog: &'a Catalog, id: Option<&str>) -> Result<&'a Column, Invalid> {
    id.and_then(|id| catalog.column(id))
        .ok_or(Invalid::Message("Choose a supported board column."))
}

fn valid_value(column: &Column, value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if value.encode_utf16().count() > VALUE_CHARACTERS {
        return false;
    }
    match column.kind.as_str() {
        "text" | "people" | "product" => true,
        "number" => value.parse::<f64>().is_ok(),
        "date" => valid_date(value),
        _ => column.choices.iter().any(|choice| choice.value == value),
    }
}

fn valid_date(value: &str) -> bool {
    if !value.is_ascii() {
        return false;
    }
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Ok(year) = value[..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..].parse::<u32>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn conditions_match(
    conditions: &[Condition],
    mode: ConditionMode,
    matches: impl Fn(&Condition) -> bool,
) -> bool {
    conditions.is_empty()
        || if mode == ConditionMode::Any {
            conditions.iter().any(matches)
        } else {
            conditions.iter().all(matches)
        }
}

pub fn validate(definition: &Definition, catalog: &Catalog) -> Valid {
    let action_groups = &definition.action_groups;
    require(
        action_groups.len() <= MAXIMUM_ACTION_GROUPS,
        "Use at most eight conditional action groups.",
    )?;
    for group in action_groups {
        require(
            !group.conditions.is_empty()
                && group.conditions.len() <= MAXIMUM_CONDITIONS_PER_GROUP
                && !group.actions.is_empty()
                && group.actions.len() <= MAXIMUM_ACTIONS,
            "Complete each conditional group with one to eight conditions and actions.",
        )?;
    }
    require(
        definition.conditions.len()
            + action_groups
                .iter()
                .map(|group| group.conditions.len())
                .sum::<usize>()
            <= MAXIMUM_CONDITIONS,
        "Use at most thirty conditions across the rule and its conditional groups.",
    )?;
    let trigger = &definition.trigger;
    if trigger.kind == TriggerKind::ButtonClicked {
        require(
            column(catalog, trigger.column_id.as_deref())?.kind == "button"
                && trigger.value.is_none()
                && trigger.previous_value.is_none(),
            "Choose the Button column that runs this automation.",
        )?;
    }
    if matches!(
        trigger.kind,
        TriggerKind::UpdatePosted | TriggerKind::ReplyPosted | TriggerKind::ReactionAdded
    ) {
        require(
            trigger.column_id.is_none() && trigger.value.is_none(),
            "Conversation triggers do not use column or value filters. Add board-column conditions to the recipe instead.",
        )?;
    }
    if trigger.kind == TriggerKind::PersonAssigned {
        require(
            column(
                catalog,
                Some(trigger.column_id.as_deref().unwrap_or("Assigned")),
            )?
            .kind
                == "people",
            "Choose a people column for this assignment trigger.",
        )?;
    }
    if trigger.kind == TriggerKind::ColumnChanged {
        let target = column(catalog, trigger.column_id.as_deref())?;
        require(
            trigger.value.is_none()
                || trigger.value.as_deref() == Some("")
                    && target.kind != "product"
                    && target.kind != "checkbox"
                || valid_value(target, trigger.value.as_deref()),
            "Choose a valid trigger value.",
        )?;
        require(
            trigger.previous_value.is_none()
                || trigger.previous_value.as_deref() == Some("")
                    && target.kind != "product"
                    && target.kind != "checkbox"
                || valid_value(target, trigger.previous_value.as_deref()),
            "Choose a valid previous trigger value.",
        )?;
    }
    require(
        definition.conditions.len() <= MAXIMUM_CONDITIONS_PER_GROUP,
        "Use at most eight conditions.",
    )?;
    for condition in definition.all_conditions() {
        let target = column(catalog, Some(&condition.column_id))?;
        if matches!(
            condition.operator,
            ConditionOperator::Is | ConditionOperator::IsNot
        ) {
            require(
                valid_value(target, condition.value.as_deref()),
                "Choose a valid condition value.",
            )?;
        }
    }
    let actions: Vec<_> = definition.all_actions().collect();
    require(
        !actions.is_empty() && actions.len() <= MAXIMUM_ACTIONS,
        "Choose between one and eight total actions, including conditional groups.",
    )?;
    for action in actions {
        require(
            action.kind == ActionKind::SetDate || action.date_basis() == DateBasis::Today,
            "A date basis applies only to set-date actions.",
        )?;
        match action.kind {
            ActionKind::AssignPeople | ActionKind::ReplacePeople => {
                let target = column(
                    catalog,
                    Some(action.column_id.as_deref().unwrap_or("Assigned")),
                )?;
                require(
                    target.kind == "people",
                    "Choose a people column for this assignment action.",
                )?;
                require(
                    !action.person_ids.is_empty()
                        && action.person_ids.len() <= 12
                        && action
                            .person_ids
                            .iter()
                            .all(|id| target.choices.iter().any(|choice| &choice.value == id)),
                    "Choose assignable people on this board.",
                )?;
            }
            ActionKind::ClearPeople => {
                require(
                    column(
                        catalog,
                        Some(action.column_id.as_deref().unwrap_or("Assigned")),
                    )?
                    .kind
                        == "people",
                    "Choose a people column to clear.",
                )?;
            }
            ActionKind::AssignCreator => {
                require(
                    column(catalog, action.column_id.as_deref())?.kind == "people",
                    "Choose a people column for the creator.",
                )?;
            }
            ActionKind::SetDate | ActionKind::PushDate => {
                require(
                    column(catalog, action.column_id.as_deref())?.kind == "date"
                        && (-MAXIMUM_OFFSET_DAYS..=MAXIMUM_OFFSET_DAYS).contains(&action.days),
                    "Choose a date column and an offset between -3650 and 3650 days.",
                )?;
            }
            ActionKind::MoveToGroup => {
                require(
                    valid_value(column(catalog, Some("Group"))?, action.value.as_deref()),
                    "Choose a group on this board.",
                )?;
            }
            ActionKind::ChangeStatus => {
                let target = column(catalog, action.column_id.as_deref())?;
                require(
                    matches!(target.kind.as_str(), "status" | "product")
                        && valid_value(target, action.value.as_deref()),
                    "Choose a supported status column and label.",
                )?;
            }
            ActionKind::Notify => {
                let message = action.message.as_deref().unwrap_or("");
                require(
                    !message.trim().is_empty()
                        && message.encode_utf16().count() <= MESSAGE_CHARACTERS
                        && action
                            .title
                            .as_deref()
                            .is_none_or(|title| title.encode_utf16().count() <= TITLE_CHARACTERS),
                    "Write a notification of 1–4,000 characters and a title of up to 160 characters.",
                )?;
                if action.recipient() == Recipient::Assigned {
                    require(
                        column(
                            catalog,
                            Some(action.column_id.as_deref().unwrap_or("Assigned")),
                        )?
                        .kind
                            == "people",
                        "Choose a people column for assigned notification recipients.",
                    )?;
                }
                if action.recipient() == Recipient::People {
                    let available: HashSet<_> =
                        catalog.people.iter().map(|person| &person.id).collect();
                    require(
                        !action.person_ids.is_empty()
                            && action.person_ids.len() <= 12
                            && action.person_ids.iter().all(|id| available.contains(id)),
                        "Choose available local people or teams as notification recipients.",
                    )?;
                }
                if action.recipient() == Recipient::TriggerPeople {
                    require(
                        matches!(
                            trigger.kind,
                            TriggerKind::PersonAssigned
                                | TriggerKind::PersonMentioned
                                | TriggerKind::UpdatePosted
                                | TriggerKind::ReplyPosted
                                | TriggerKind::ReactionAdded
                        ),
                        "Trigger recipients are available for assignment, mention and conversation triggers.",
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Store the editor-facing sentence for every clause in one pass.
pub struct Sentences {
    pub trigger: String,
    pub conditions: Vec<String>,
    pub actions: Vec<String>,
}

pub fn sentences(definition: &Definition, catalog: &Catalog) -> Sentences {
    let column = |id: &str| {
        catalog
            .column(id)
            .map(|column| column.name.as_str())
            .unwrap_or("")
    };
    let value = |id: &str, value: Option<&str>| -> String {
        catalog
            .column(id)
            .and_then(|column| {
                column
                    .choices
                    .iter()
                    .find(|choice| Some(choice.value.as_str()) == value)
            })
            .map(|choice| choice.name.clone())
            .unwrap_or_else(|| {
                value
                    .filter(|value| !value.is_empty())
                    .unwrap_or("empty")
                    .to_owned()
            })
    };
    let trigger = &definition.trigger;
    let trigger_text = match trigger.kind {
        TriggerKind::ItemCreated => "When an item is created".to_owned(),
        TriggerKind::ButtonClicked => {
            format!(
                "When {} is clicked",
                column(trigger.column_id.as_deref().unwrap_or(""))
            )
        }
        TriggerKind::PersonAssigned => "When a person is assigned".to_owned(),
        TriggerKind::PersonMentioned => {
            "When a person is mentioned in an update or reply".to_owned()
        }
        TriggerKind::UpdatePosted => "When an update is posted".to_owned(),
        TriggerKind::ReplyPosted => "When a reply is posted".to_owned(),
        TriggerKind::ReactionAdded => "When a reaction is added".to_owned(),
        TriggerKind::ColumnChanged => {
            let mut text = format!(
                "When {} changes",
                column(trigger.column_id.as_deref().unwrap_or(""))
            );
            if let Some(previous) = &trigger.previous_value {
                text.push_str(" from ");
                text.push_str(&value(
                    trigger.column_id.as_deref().unwrap_or(""),
                    Some(previous),
                ));
            }
            if let Some(next) = &trigger.value {
                text.push_str(" to ");
                text.push_str(&value(
                    trigger.column_id.as_deref().unwrap_or(""),
                    Some(next),
                ));
            }
            text
        }
    };
    let condition = |condition: &Condition| {
        let mut text = column(&condition.column_id).to_owned();
        match condition.operator {
            ConditionOperator::Is => {
                text.push_str(" is ");
                text.push_str(&value(&condition.column_id, condition.value.as_deref()));
            }
            ConditionOperator::IsNot => {
                text.push_str(" is not ");
                text.push_str(&value(&condition.column_id, condition.value.as_deref()));
            }
            ConditionOperator::IsEmpty => text.push_str(" is empty"),
            ConditionOperator::IsNotEmpty => text.push_str(" is not empty"),
        }
        text
    };
    let names: HashMap<&str, &str> = catalog
        .people
        .iter()
        .map(|person| (person.id.as_str(), person.name.as_str()))
        .collect();
    let person = |id: &str| -> String { names.get(id).copied().unwrap_or(id).to_owned() };
    let action = |action: &Action| {
        let people = || {
            action
                .person_ids
                .iter()
                .map(|id| person(id))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match action.kind {
            ActionKind::AssignPeople => format!(
                "Add {} to {}",
                people(),
                column(action.column_id.as_deref().unwrap_or("Assigned"))
            ),
            ActionKind::ReplacePeople => format!(
                "Replace {} with {}",
                column(action.column_id.as_deref().unwrap_or("Assigned")),
                people()
            ),
            ActionKind::ClearPeople => format!(
                "Clear {}",
                column(action.column_id.as_deref().unwrap_or("Assigned"))
            ),
            ActionKind::AssignCreator => {
                format!(
                    "Assign the creator as {}",
                    column(action.column_id.as_deref().unwrap_or(""))
                )
            }
            ActionKind::SetDate => format!(
                "Set {} to {}{}",
                column(action.column_id.as_deref().unwrap_or("")),
                if action.date_basis() == DateBasis::ItemCreation {
                    "item creation date"
                } else {
                    "today"
                },
                if action.days == 0 {
                    String::new()
                } else {
                    format!(" plus {} days", action.days)
                }
            ),
            ActionKind::PushDate => format!(
                "Push {} by {} days",
                column(action.column_id.as_deref().unwrap_or("")),
                action.days
            ),
            ActionKind::MoveToGroup => {
                format!("Move item to {}", value("Group", action.value.as_deref()))
            }
            ActionKind::ChangeStatus => format!(
                "Set {} to {}",
                column(action.column_id.as_deref().unwrap_or("")),
                value(
                    action.column_id.as_deref().unwrap_or(""),
                    action.value.as_deref()
                )
            ),
            ActionKind::Notify => format!(
                "Notify {}: {}",
                if action.recipient() == Recipient::People {
                    people()
                } else {
                    match action.recipient() {
                        Recipient::Assigned => "Assigned".to_owned(),
                        Recipient::Creator => "Creator".to_owned(),
                        Recipient::TriggerPeople => "TriggerPeople".to_owned(),
                        Recipient::People => String::new(),
                    }
                },
                action.message.as_deref().unwrap_or("")
            ),
        }
    };
    let conditions = if definition.conditions.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "Match {}: {}",
            if definition.conditions_mode == ConditionMode::Any {
                "any"
            } else {
                "all"
            },
            definition
                .conditions
                .iter()
                .map(condition)
                .collect::<Vec<_>>()
                .join(", ")
        )]
    };
    let actions = definition
        .actions
        .iter()
        .map(action)
        .chain(definition.action_groups.iter().map(|group| {
            format!(
                "If {} ({}), {}",
                if group.mode == ConditionMode::Any {
                    "any"
                } else {
                    "all"
                },
                group
                    .conditions
                    .iter()
                    .map(condition)
                    .collect::<Vec<_>>()
                    .join(", "),
                group
                    .actions
                    .iter()
                    .map(action)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }))
        .collect();
    Sentences {
        trigger: trigger_text,
        conditions,
        actions,
    }
}

pub fn assignee_ids(definition: &Definition) -> Vec<String> {
    let mut seen = HashSet::new();
    definition
        .all_actions()
        .filter(|action| action.kind == ActionKind::AssignPeople)
        .flat_map(|action| action.person_ids.iter().cloned())
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

pub fn condition_matches(condition: &Condition, values: &HashMap<String, Vec<String>>) -> bool {
    let actual = values.get(&condition.column_id);
    match condition.operator {
        ConditionOperator::Is => actual.is_some_and(|actual| {
            actual
                .iter()
                .any(|value| Some(value) == condition.value.as_ref())
        }),
        ConditionOperator::IsNot => !actual.is_some_and(|actual| {
            actual
                .iter()
                .any(|value| Some(value) == condition.value.as_ref())
        }),
        ConditionOperator::IsEmpty => actual.is_none_or(Vec::is_empty),
        ConditionOperator::IsNotEmpty => actual.is_some_and(|actual| !actual.is_empty()),
    }
}

pub fn conditions_match_all(
    conditions: &[Condition],
    mode: ConditionMode,
    values: &HashMap<String, Vec<String>>,
) -> bool {
    conditions_match(conditions, mode, |condition| {
        condition_matches(condition, values)
    })
}
