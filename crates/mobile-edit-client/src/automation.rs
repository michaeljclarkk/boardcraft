//! Automation definitions, rules and run history shared by every client.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTriggerKind {
    ItemCreated,
    ButtonClicked,
    ColumnChanged,
    PersonAssigned,
    PersonMentioned,
    UpdatePosted,
    ReplyPosted,
    ReactionAdded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationConditionOperator {
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationActionKind {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRecipient {
    People,
    Assigned,
    Creator,
    TriggerPeople,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationDateBasis {
    Today,
    ItemCreation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationConditionMode {
    #[default]
    All,
    Any,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationTrigger {
    pub kind: AutomationTriggerKind,
    #[serde(default)]
    pub column_id: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub previous_value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationCondition {
    pub column_id: String,
    pub operator: AutomationConditionOperator,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationAction {
    pub kind: AutomationActionKind,
    #[serde(default)]
    pub column_id: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub person_ids: Vec<String>,
    #[serde(default)]
    pub days: i32,
    #[serde(default)]
    pub recipient: Option<AutomationRecipient>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub date_basis: Option<AutomationDateBasis>,
}

impl AutomationAction {
    pub fn recipient(&self) -> AutomationRecipient {
        self.recipient.unwrap_or(AutomationRecipient::People)
    }

    pub fn date_basis(&self) -> AutomationDateBasis {
        self.date_basis.unwrap_or(AutomationDateBasis::Today)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationActionGroup {
    pub conditions: Vec<AutomationCondition>,
    pub actions: Vec<AutomationAction>,
    #[serde(default)]
    pub mode: AutomationConditionMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationDefinition {
    pub trigger: AutomationTrigger,
    #[serde(default)]
    pub conditions: Vec<AutomationCondition>,
    #[serde(default)]
    pub actions: Vec<AutomationAction>,
    #[serde(default)]
    pub conditions_mode: AutomationConditionMode,
    #[serde(default)]
    pub action_groups: Vec<AutomationActionGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationChoice {
    pub value: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationColumn {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub choices: Vec<AutomationChoice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationPerson {
    pub id: String,
    pub name: String,
    pub can_assign: bool,
    pub can_mention: bool,
    pub team: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationCatalog {
    pub board_name: String,
    pub columns: Vec<AutomationColumn>,
    pub triggers: Vec<AutomationTriggerKind>,
    pub actions: Vec<AutomationActionKind>,
    pub maximum_conditions: i64,
    pub maximum_actions: i64,
    pub maximum_rules: i64,
    pub people: Vec<AutomationPerson>,
    pub items: Vec<AutomationItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDisposition {
    Ready,
    ReviewRequired,
    Obsolete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationRule {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub conditions: Vec<String>,
    pub actions: Vec<String>,
    pub enabled: bool,
    pub revision: i64,
    pub disposition: RuleDisposition,
    pub dependency_issue: Option<String>,
    pub product_column_id: Option<String>,
    pub assignee_ids: Vec<String>,
    pub source_recipes: Vec<String>,
    pub evidence: Vec<String>,
    pub decision: String,
    pub canonical_rule_id: Option<String>,
    pub definition: Option<AutomationDefinition>,
    pub position: i64,
    pub archived: bool,
    pub created_by: String,
    pub created_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationOutcome {
    Succeeded,
    SkippedDisabled,
    ConditionNotMet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationRun {
    pub id: String,
    pub idempotency_key: String,
    pub rule_id: String,
    pub rule_name: String,
    pub rule_revision: i64,
    pub item_id: String,
    pub item_revision: i64,
    pub actor_id: String,
    pub outcome: AutomationOutcome,
    pub trigger_snapshot: String,
    pub action_snapshot: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationHistory {
    pub runs: Vec<AutomationRun>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveAutomationRule {
    pub id: String,
    pub expected_revision: i64,
    pub name: String,
    pub enabled: bool,
    pub definition: AutomationDefinition,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct EnabledEdit {
    expected_revision: i64,
    enabled: bool,
}
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ArchivedEdit {
    expected_revision: i64,
    archived: bool,
}
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    rule_id: Option<String>,
    offset: i64,
}

#[cfg(not(target_family = "wasm"))]
impl super::EditClient {
    pub fn automation_catalog(
        &self,
        board: uuid::Uuid,
    ) -> Result<AutomationCatalog, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/catalog",
            self.port
        );
        super::decode(self.authorize(self.http.get(url)).call())
    }

    pub fn list_automation_rules(
        &self,
        board: uuid::Uuid,
        archived: bool,
        offset: i64,
    ) -> Result<Vec<AutomationRule>, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/rules?archived={archived}&offset={offset}",
            self.port
        );
        super::decode(self.authorize(self.http.get(url)).call())
    }

    pub fn save_automation_rule(
        &self,
        board: uuid::Uuid,
        input: &SaveAutomationRule,
    ) -> Result<AutomationRule, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/rules",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(input))
    }

    pub fn set_automation_rule_enabled(
        &self,
        board: uuid::Uuid,
        id: &str,
        expected_revision: i64,
        enabled: bool,
    ) -> Result<AutomationRule, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/rules/{id}/enabled",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(EnabledEdit {
            expected_revision,
            enabled,
        }))
    }

    pub fn set_automation_rule_archived(
        &self,
        board: uuid::Uuid,
        id: &str,
        expected_revision: i64,
        archived: bool,
    ) -> Result<AutomationRule, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/rules/{id}/archived",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(ArchivedEdit {
            expected_revision,
            archived,
        }))
    }

    pub fn automation_history(
        &self,
        board: uuid::Uuid,
        rule_id: Option<&str>,
        offset: i64,
    ) -> Result<AutomationHistory, super::EditError> {
        let url = format!(
            "http://127.0.0.1:{}/navigation/boards/{board}/automation/history",
            self.port
        );
        super::decode(self.authorize(self.http.post(url)).send_json(HistoryQuery {
            rule_id: rule_id.map(str::to_owned),
            offset,
        }))
    }
}
