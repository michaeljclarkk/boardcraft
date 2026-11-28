//! Typed, closed automation definitions shared by the editor and the executor.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    ItemCreated,
    ButtonClicked,
    ColumnChanged,
    PersonAssigned,
    PersonMentioned,
    UpdatePosted,
    ReplyPosted,
    ReactionAdded,
}

impl TriggerKind {
    pub const ALL: [Self; 8] = [
        Self::ItemCreated,
        Self::ButtonClicked,
        Self::ColumnChanged,
        Self::PersonAssigned,
        Self::PersonMentioned,
        Self::UpdatePosted,
        Self::ReplyPosted,
        Self::ReactionAdded,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    AssignPeople,
    AssignCreator,
    SetDate,
    PushDate,
    MoveToGroup,
    ChangeStatus,
    Notify,
    ReplacePeople,
    ClearPeople,
}

impl ActionKind {
    pub const ALL: [Self; 9] = [
        Self::AssignPeople,
        Self::AssignCreator,
        Self::SetDate,
        Self::PushDate,
        Self::MoveToGroup,
        Self::ChangeStatus,
        Self::Notify,
        Self::ReplacePeople,
        Self::ClearPeople,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recipient {
    People,
    Assigned,
    Creator,
    TriggerPeople,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateBasis {
    Today,
    ItemCreation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionMode {
    #[default]
    All,
    Any,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub kind: TriggerKind,
    #[serde(default)]
    pub column_id: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub previous_value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub column_id: String,
    pub operator: ConditionOperator,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub kind: ActionKind,
    #[serde(default)]
    pub column_id: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub person_ids: Vec<String>,
    #[serde(default)]
    pub days: i32,
    #[serde(default)]
    pub recipient: Option<Recipient>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub date_basis: Option<DateBasis>,
}

impl Action {
    pub fn recipient(&self) -> Recipient {
        self.recipient.unwrap_or(Recipient::People)
    }

    pub fn date_basis(&self) -> DateBasis {
        self.date_basis.unwrap_or(DateBasis::Today)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionGroup {
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub mode: ConditionMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub trigger: Trigger,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub conditions_mode: ConditionMode,
    #[serde(default)]
    pub action_groups: Vec<ActionGroup>,
}

impl Definition {
    pub fn all_actions(&self) -> impl Iterator<Item = &Action> {
        self.actions
            .iter()
            .chain(self.action_groups.iter().flat_map(|group| &group.actions))
    }

    pub fn all_conditions(&self) -> impl Iterator<Item = &Condition> {
        self.conditions.iter().chain(
            self.action_groups
                .iter()
                .flat_map(|group| &group.conditions),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub choices: Vec<Choice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub can_assign: bool,
    pub can_mention: bool,
    pub team: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub board_name: String,
    pub columns: Vec<Column>,
    pub triggers: Vec<TriggerKind>,
    pub actions: Vec<ActionKind>,
    pub maximum_conditions: i64,
    pub maximum_actions: i64,
    pub maximum_rules: i64,
    pub people: Vec<Person>,
    pub items: Vec<ItemRef>,
}

impl Catalog {
    pub fn column(&self, id: &str) -> Option<&Column> {
        self.columns.iter().find(|column| column.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Ready,
    ReviewRequired,
    Obsolete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub conditions: Vec<String>,
    pub actions: Vec<String>,
    pub enabled: bool,
    pub revision: i64,
    pub disposition: Disposition,
    pub dependency_issue: Option<String>,
    pub product_column_id: Option<String>,
    pub assignee_ids: Vec<String>,
    pub source_recipes: Vec<String>,
    pub evidence: Vec<String>,
    pub decision: String,
    pub canonical_rule_id: Option<String>,
    pub definition: Option<Definition>,
    pub position: i64,
    pub archived: bool,
    pub created_by: String,
    pub created_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Succeeded,
    SkippedDisabled,
    ConditionNotMet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub id: String,
    pub idempotency_key: String,
    pub rule_id: String,
    pub rule_name: String,
    pub rule_revision: i64,
    pub item_id: String,
    pub item_revision: i64,
    pub actor_id: String,
    pub outcome: Outcome,
    pub trigger_snapshot: String,
    pub action_snapshot: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub runs: Vec<Run>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveInput {
    pub id: String,
    pub expected_revision: i64,
    pub name: String,
    pub enabled: bool,
    pub definition: Definition,
}
