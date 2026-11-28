//! Automation management panel and recipe wizard for the desktop client.
//!
//! The panel owns the automation catalogue, the rule list, run history and one
//! retained editor draft. The editor is kept while the panel is open so that
//! closing it with Back and reopening it returns the same draft; nothing is
//! written to browser storage.
use boardcraft_mobile_edit_client::{
    AutomationAction, AutomationActionGroup, AutomationActionKind, AutomationCatalog,
    AutomationChoice, AutomationCondition, AutomationConditionMode, AutomationConditionOperator,
    AutomationDateBasis, AutomationDefinition, AutomationRecipient, AutomationRule, AutomationRun,
    AutomationTrigger, AutomationTriggerKind, EditClient, EditError, RuleDisposition,
    SaveAutomationRule,
};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::Button,
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::*;
use std::rc::Rc;
use uuid::Uuid;

const MAX_GROUPS: usize = 8;
const RULE_PAGE: i64 = 250;

/// Selected view inside the automation panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomationTab {
    Manage,
    RunHistory,
}

/// Emitted when the panel should close.
pub struct AutomationClosed;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadState {
    Idle,
    Loading,
    Loaded,
    Failed,
}

// ---------------------------------------------------------------------------
// Labels and small pure helpers
// ---------------------------------------------------------------------------

fn trigger_label(kind: AutomationTriggerKind) -> &'static str {
    match kind {
        AutomationTriggerKind::ItemCreated => "an item is created",
        AutomationTriggerKind::ButtonClicked => "a button is clicked",
        AutomationTriggerKind::ColumnChanged => "a column changes",
        AutomationTriggerKind::PersonAssigned => "a person is assigned",
        AutomationTriggerKind::PersonMentioned => "a person is mentioned",
        AutomationTriggerKind::UpdatePosted => "an update is posted",
        AutomationTriggerKind::ReplyPosted => "a reply is posted",
        AutomationTriggerKind::ReactionAdded => "a reaction is added",
    }
}

fn action_label(kind: AutomationActionKind) -> &'static str {
    match kind {
        AutomationActionKind::AssignPeople => "add assignee",
        AutomationActionKind::ReplacePeople => "replace assignees",
        AutomationActionKind::ClearPeople => "clear assignees",
        AutomationActionKind::AssignCreator => "assign item creator",
        AutomationActionKind::SetDate => "set date",
        AutomationActionKind::PushDate => "push date",
        AutomationActionKind::MoveToGroup => "move item to group",
        AutomationActionKind::ChangeStatus => "change status",
        AutomationActionKind::Notify => "notify someone",
    }
}

fn operator_label(operator: AutomationConditionOperator) -> &'static str {
    match operator {
        AutomationConditionOperator::Is => "is",
        AutomationConditionOperator::IsNot => "is not",
        AutomationConditionOperator::IsEmpty => "is empty",
        AutomationConditionOperator::IsNotEmpty => "is not empty",
    }
}

fn disposition_label(disposition: RuleDisposition) -> &'static str {
    match disposition {
        RuleDisposition::Ready => "Ready",
        RuleDisposition::ReviewRequired => "Review required",
        RuleDisposition::Obsolete => "Obsolete",
    }
}

fn outcome_label(outcome: boardcraft_mobile_edit_client::AutomationOutcome) -> &'static str {
    match outcome {
        boardcraft_mobile_edit_client::AutomationOutcome::Succeeded => "Succeeded",
        boardcraft_mobile_edit_client::AutomationOutcome::SkippedDisabled => "Skipped disabled",
        boardcraft_mobile_edit_client::AutomationOutcome::ConditionNotMet => "Condition not met",
    }
}

fn mode_label(mode: AutomationConditionMode) -> &'static str {
    match mode {
        AutomationConditionMode::All => "all conditions match (AND)",
        AutomationConditionMode::Any => "any condition matches (OR)",
    }
}

fn recipient_label(recipient: AutomationRecipient, trigger: Option<AutomationTriggerKind>) -> &'static str {
    match recipient {
        AutomationRecipient::People => "people",
        AutomationRecipient::Assigned => "Assigned",
        AutomationRecipient::Creator => "item creator",
        AutomationRecipient::TriggerPeople => match trigger {
            Some(AutomationTriggerKind::PersonAssigned) => "newly assigned people",
            Some(AutomationTriggerKind::PersonMentioned) => "newly mentioned people",
            Some(AutomationTriggerKind::ReactionAdded) => "person who reacted",
            _ => "person who posted",
        },
    }
}

fn trigger_kind_from(value: &str) -> Option<AutomationTriggerKind> {
    Some(match value {
        "item_created" => AutomationTriggerKind::ItemCreated,
        "button_clicked" => AutomationTriggerKind::ButtonClicked,
        "column_changed" => AutomationTriggerKind::ColumnChanged,
        "person_assigned" => AutomationTriggerKind::PersonAssigned,
        "person_mentioned" => AutomationTriggerKind::PersonMentioned,
        "update_posted" => AutomationTriggerKind::UpdatePosted,
        "reply_posted" => AutomationTriggerKind::ReplyPosted,
        "reaction_added" => AutomationTriggerKind::ReactionAdded,
        _ => return None,
    })
}

fn action_kind_from(value: &str) -> Option<AutomationActionKind> {
    Some(match value {
        "assign_people" => AutomationActionKind::AssignPeople,
        "assign_creator" => AutomationActionKind::AssignCreator,
        "set_date" => AutomationActionKind::SetDate,
        "push_date" => AutomationActionKind::PushDate,
        "move_to_group" => AutomationActionKind::MoveToGroup,
        "change_status" => AutomationActionKind::ChangeStatus,
        "notify" => AutomationActionKind::Notify,
        "replace_people" => AutomationActionKind::ReplacePeople,
        "clear_people" => AutomationActionKind::ClearPeople,
        _ => return None,
    })
}

fn operator_from(value: &str) -> Option<AutomationConditionOperator> {
    Some(match value {
        "is" => AutomationConditionOperator::Is,
        "is_not" => AutomationConditionOperator::IsNot,
        "is_empty" => AutomationConditionOperator::IsEmpty,
        "is_not_empty" => AutomationConditionOperator::IsNotEmpty,
        _ => return None,
    })
}

fn mode_from(value: &str) -> Option<AutomationConditionMode> {
    match value {
        "all" => Some(AutomationConditionMode::All),
        "any" => Some(AutomationConditionMode::Any),
        _ => None,
    }
}

fn recipient_from(value: &str) -> Option<AutomationRecipient> {
    Some(match value {
        "people" => AutomationRecipient::People,
        "assigned" => AutomationRecipient::Assigned,
        "creator" => AutomationRecipient::Creator,
        "trigger_people" => AutomationRecipient::TriggerPeople,
        _ => return None,
    })
}

fn date_basis_from(value: &str) -> Option<AutomationDateBasis> {
    Some(match value {
        "today" => AutomationDateBasis::Today,
        "item_creation" => AutomationDateBasis::ItemCreation,
        _ => return None,
    })
}

fn error_text(error: &EditError) -> &'static str {
    match error {
        EditError::SessionRejected => "Session expired. Sign in again.",
        EditError::PermissionDenied => "Permission denied.",
        EditError::AccessUnavailable => "Access check unavailable. Try again.",
        EditError::InvalidDraft => "Enter a valid automation.",
        EditError::Conflict => "This automation changed. Reload latest.",
        EditError::Missing => "This automation no longer exists.",
        EditError::HostFault => "The server could not complete the request.",
        EditError::Transport => "Could not reach the server.",
    }
}

fn text_value(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn choice(value: impl Into<String>, name: impl Into<String>) -> AutomationChoice {
    AutomationChoice {
        value: value.into(),
        name: name.into(),
    }
}

fn column_choices(catalog: Option<&AutomationCatalog>) -> Vec<AutomationChoice> {
    catalog
        .map(|catalog| {
            catalog
                .columns
                .iter()
                .map(|column| choice(column.id.clone(), column.name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn named_columns(catalog: Option<&AutomationCatalog>, kind: &str) -> Vec<AutomationChoice> {
    catalog
        .map(|catalog| {
            catalog
                .columns
                .iter()
                .filter(|column| column.kind == kind)
                .map(|column| choice(column.id.clone(), column.name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn rule_sentence(rule: &AutomationRule) -> String {
    let mut sentence = if rule.trigger.starts_with("When ") {
        rule.trigger.clone()
    } else {
        let mut chars = rule.trigger.chars();
        match chars.next() {
            Some(first) => format!("When {}{}", first.to_lowercase(), chars.as_str()),
            None => "When".to_owned(),
        }
    };
    if !rule.conditions.is_empty() {
        sentence.push_str(", and only if ");
        sentence.push_str(&rule.conditions.join(" and "));
    }
    sentence.push_str(", then ");
    sentence.push_str(&rule.actions.join(", then "));
    sentence
}

fn snapshot_text(snapshot: &str, field: &str, fallback: &str) -> Vec<String> {
    let parsed: Option<serde_json::Value> = serde_json::from_str(snapshot).ok();
    if let Some(serde_json::Value::Object(map)) = parsed
        && let Some(value) = map.get(field)
    {
        match value {
            serde_json::Value::String(text) => return vec![text.clone()],
            serde_json::Value::Array(entries) => {
                let text: Vec<String> = entries
                    .iter()
                    .filter_map(|entry| entry.as_str().map(str::to_owned))
                    .collect();
                if !text.is_empty() {
                    return text;
                }
            }
            _ => (),
        }
    }
    vec![fallback.to_owned()]
}

fn item_name(catalog: Option<&AutomationCatalog>, id: &str) -> String {
    catalog
        .and_then(|catalog| catalog.items.iter().find(|item| item.id == id))
        .map(|item| item.name.clone())
        .unwrap_or_else(|| format!("Item {}", id.chars().take(8).collect::<String>()))
}

fn person_name(catalog: Option<&AutomationCatalog>, id: &str) -> String {
    catalog
        .and_then(|catalog| catalog.people.iter().find(|person| person.id == id))
        .map(|person| person.name.clone())
        .unwrap_or_else(|| id.to_owned())
}

fn format_timestamp(raw: &str) -> String {
    let text = raw.trim_end_matches('Z');
    if text.len() >= 16
        && text.as_bytes()[4] == b'-'
        && text.as_bytes()[7] == b'-'
        && text.as_bytes()[10] == b'T'
    {
        format!("{} {}", &text[..10], &text[11..16])
    } else {
        text.to_owned()
    }
}

fn column_choice_kind<'a>(
    catalog: Option<&'a AutomationCatalog>,
    id: Option<&str>,
) -> Option<&'a str> {
    let id = id?;
    catalog?
        .columns
        .iter()
        .find(|column| column.id == id)
        .map(|column| column.kind.as_str())
}

fn is_text_kind(kind: Option<&str>) -> bool {
    matches!(kind, Some("text" | "date" | "number"))
}

fn input_for(
    window: &mut Window,
    cx: &mut Context<AutomationPanel>,
    value: &str,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).default_value(value.to_owned()))
}

// ---------------------------------------------------------------------------
// Retained editor draft
// ---------------------------------------------------------------------------

struct ConditionDraft {
    column_id: Option<String>,
    operator: AutomationConditionOperator,
    value: Option<String>,
    input: Entity<InputState>,
}

struct ActionDraft {
    kind: AutomationActionKind,
    column_id: Option<String>,
    value: Option<String>,
    person_ids: Vec<String>,
    days_input: Entity<InputState>,
    date_basis: AutomationDateBasis,
    recipient: AutomationRecipient,
    title: Option<String>,
    message: Option<String>,
    input: Entity<InputState>,
}

struct GroupDraft {
    mode: AutomationConditionMode,
    conditions: Vec<ConditionDraft>,
    actions: Vec<ActionDraft>,
}

struct TriggerDraft {
    kind: AutomationTriggerKind,
    column_id: Option<String>,
    value: Option<String>,
    previous_value: Option<String>,
    value_input: Entity<InputState>,
    previous_input: Entity<InputState>,
}

struct Composer {
    group: Option<usize>,
    index: usize,
    title: Entity<InputState>,
    message: Entity<TextareaState>,
}

#[derive(Clone)]
struct EditorSeed {
    rule_id: String,
    expected_revision: i64,
    name: String,
    enabled: bool,
    trigger_chosen: bool,
    definition: AutomationDefinition,
}

struct Editor {
    seed: EditorSeed,
    name: Entity<InputState>,
    trigger_chosen: bool,
    trigger: TriggerDraft,
    conditions: Vec<ConditionDraft>,
    conditions_mode: AutomationConditionMode,
    actions: Vec<ActionDraft>,
    groups: Vec<GroupDraft>,
    enabled: bool,
    saving: bool,
    error: Option<String>,
    composer: Option<Composer>,
}

impl Editor {
    fn new(seed: EditorSeed, window: &mut Window, cx: &mut Context<AutomationPanel>) -> Self {
        let name = input_for(window, cx, &seed.name);
        let definition = seed.definition.clone();
        let trigger = TriggerDraft {
            kind: definition.trigger.kind,
            column_id: definition.trigger.column_id.clone(),
            value: definition.trigger.value.clone(),
            previous_value: definition.trigger.previous_value.clone(),
            value_input: input_for(window, cx, definition.trigger.value.as_deref().unwrap_or("")),
            previous_input: input_for(
                window,
                cx,
                definition.trigger.previous_value.as_deref().unwrap_or(""),
            ),
        };
        let conditions = definition
            .conditions
            .iter()
            .map(|condition| {
                let input = input_for(window, cx, condition.value.as_deref().unwrap_or(""));
                ConditionDraft {
                    column_id: Some(condition.column_id.clone()),
                    operator: condition.operator,
                    value: condition.value.clone(),
                    input,
                }
            })
            .collect();
        let actions = definition
            .actions
            .iter()
            .map(|action| action_draft_from(action, window, cx))
            .collect();
        let groups = definition
            .action_groups
            .iter()
            .map(|group| GroupDraft {
                mode: group.mode,
                conditions: group
                    .conditions
                    .iter()
                    .map(|condition| {
                        let input =
                            input_for(window, cx, condition.value.as_deref().unwrap_or(""));
                        ConditionDraft {
                            column_id: Some(condition.column_id.clone()),
                            operator: condition.operator,
                            value: condition.value.clone(),
                            input,
                        }
                    })
                    .collect(),
                actions: group
                    .actions
                    .iter()
                    .map(|action| action_draft_from(action, window, cx))
                    .collect(),
            })
            .collect();
        Self {
            trigger_chosen: seed.trigger_chosen,
            name,
            trigger,
            conditions,
            conditions_mode: definition.conditions_mode,
            actions,
            groups,
            enabled: seed.enabled,
            saving: false,
            error: None,
            composer: None,
            seed,
        }
    }

    fn is_existing(&self) -> bool {
        self.seed.expected_revision > 0
    }

    fn total_actions(&self) -> usize {
        self.actions.len() + self.groups.iter().map(|group| group.actions.len()).sum::<usize>()
    }

    fn total_conditions(&self) -> usize {
        self.conditions.len()
            + self
                .groups
                .iter()
                .map(|group| group.conditions.len())
                .sum::<usize>()
    }

    fn actions_mut(&mut self, group: Option<usize>) -> Option<&mut Vec<ActionDraft>> {
        match group {
            Some(index) => self.groups.get_mut(index).map(|group| &mut group.actions),
            None => Some(&mut self.actions),
        }
    }

    fn conditions_mut(&mut self, group: Option<usize>) -> Option<&mut Vec<ConditionDraft>> {
        match group {
            Some(index) => self.groups.get_mut(index).map(|group| &mut group.conditions),
            None => Some(&mut self.conditions),
        }
    }

    fn action(&self, group: Option<usize>, index: usize) -> Option<&ActionDraft> {
        match group {
            Some(group) => self.groups.get(group)?.actions.get(index),
            None => self.actions.get(index),
        }
    }

    fn action_mut(&mut self, group: Option<usize>, index: usize) -> Option<&mut ActionDraft> {
        match group {
            Some(group) => self.groups.get_mut(group)?.actions.get_mut(index),
            None => self.actions.get_mut(index),
        }
    }

    fn condition_mut(
        &mut self,
        group: Option<usize>,
        index: usize,
    ) -> Option<&mut ConditionDraft> {
        match group {
            Some(group) => self.groups.get_mut(group)?.conditions.get_mut(index),
            None => self.conditions.get_mut(index),
        }
    }

    fn definition(&self, catalog: Option<&AutomationCatalog>, cx: &App) -> AutomationDefinition {
        let trigger_kind = column_choice_kind(catalog, self.trigger.column_id.as_deref());
        let text_trigger = self.trigger.kind == AutomationTriggerKind::ColumnChanged
            && is_text_kind(trigger_kind);
        let conditions = self
            .conditions
            .iter()
            .map(|condition| condition_value(condition, catalog, cx))
            .collect();
        let actions = self
            .actions
            .iter()
            .map(|action| action_value(action, catalog, cx))
            .collect();
        let action_groups = self
            .groups
            .iter()
            .map(|group| AutomationActionGroup {
                mode: group.mode,
                conditions: group
                    .conditions
                    .iter()
                    .map(|condition| condition_value(condition, catalog, cx))
                    .collect(),
                actions: group
                    .actions
                    .iter()
                    .map(|action| action_value(action, catalog, cx))
                    .collect(),
            })
            .collect();
        AutomationDefinition {
            trigger: AutomationTrigger {
                kind: self.trigger.kind,
                column_id: self.trigger.column_id.clone(),
                value: if text_trigger {
                    text_value(self.trigger.value_input.read(cx).value().as_ref())
                } else {
                    self.trigger.value.clone()
                },
                previous_value: if text_trigger {
                    text_value(self.trigger.previous_input.read(cx).value().as_ref())
                } else {
                    self.trigger.previous_value.clone()
                },
            },
            conditions,
            actions,
            conditions_mode: self.conditions_mode,
            action_groups,
        }
    }
}

fn condition_value(
    condition: &ConditionDraft,
    catalog: Option<&AutomationCatalog>,
    cx: &App,
) -> AutomationCondition {
    let kind = column_choice_kind(catalog, condition.column_id.as_deref());
    let value = match condition.operator {
        AutomationConditionOperator::Is | AutomationConditionOperator::IsNot => {
            if is_text_kind(kind) {
                text_value(condition.input.read(cx).value().as_ref())
            } else {
                condition.value.clone()
            }
        }
        AutomationConditionOperator::IsEmpty | AutomationConditionOperator::IsNotEmpty => None,
    };
    AutomationCondition {
        column_id: condition.column_id.clone().unwrap_or_default(),
        operator: condition.operator,
        value,
    }
}

fn action_draft_from(
    action: &AutomationAction,
    window: &mut Window,
    cx: &mut Context<AutomationPanel>,
) -> ActionDraft {
    let input = input_for(window, cx, action.value.as_deref().unwrap_or(""));
    let days_input = input_for(window, cx, &action.days.to_string());
    ActionDraft {
        kind: action.kind,
        column_id: action.column_id.clone(),
        value: action.value.clone(),
        person_ids: action.person_ids.clone(),
        days_input,
        date_basis: action.date_basis(),
        recipient: action.recipient(),
        title: action.title.clone(),
        message: action.message.clone(),
        input,
    }
}

fn action_value(
    action: &ActionDraft,
    catalog: Option<&AutomationCatalog>,
    cx: &App,
) -> AutomationAction {
    let kind = column_choice_kind(catalog, action.column_id.as_deref());
    let mut value = AutomationAction {
        kind: action.kind,
        column_id: None,
        value: None,
        person_ids: Vec::new(),
        days: 0,
        recipient: None,
        title: None,
        message: None,
        date_basis: None,
    };
    match action.kind {
        AutomationActionKind::AssignPeople | AutomationActionKind::ReplacePeople => {
            value.column_id = action.column_id.clone();
            value.person_ids = action.person_ids.clone();
        }
        AutomationActionKind::ClearPeople | AutomationActionKind::AssignCreator => {
            value.column_id = action.column_id.clone();
        }
        AutomationActionKind::SetDate | AutomationActionKind::PushDate => {
            value.column_id = action.column_id.clone();
            value.date_basis = Some(action.date_basis);
            value.days = action
                .days_input
                .read(cx)
                .value()
                .trim()
                .parse::<i32>()
                .unwrap_or(0);
        }
        AutomationActionKind::MoveToGroup => {
            value.value = action.value.clone();
        }
        AutomationActionKind::ChangeStatus => {
            value.column_id = action.column_id.clone();
            value.value = if is_text_kind(kind) {
                text_value(action.input.read(cx).value().as_ref())
            } else {
                action.value.clone()
            };
        }
        AutomationActionKind::Notify => {
            value.recipient = Some(action.recipient);
            value.title = action.title.clone();
            value.message = action.message.clone();
            if action.recipient == AutomationRecipient::People {
                value.person_ids = action.person_ids.clone();
            }
        }
    }
    value
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

pub struct AutomationPanel {
    client: EditClient,
    board: Uuid,
    board_name: SharedString,
    owner: bool,
    tab: AutomationTab,
    catalog: Option<AutomationCatalog>,
    catalog_state: LoadState,
    catalog_error: Option<String>,
    rules: Option<Vec<AutomationRule>>,
    rules_state: LoadState,
    rules_error: Option<String>,
    rules_archived: bool,
    rules_has_more: bool,
    search: Entity<InputState>,
    search_text: String,
    busy_rule: Option<String>,
    expanded: std::collections::HashSet<String>,
    history_rule: Option<String>,
    history_runs: Vec<AutomationRun>,
    history_state: LoadState,
    history_error: Option<String>,
    history_has_more: bool,
    history_offset: i64,
    editor: Option<Editor>,
    editor_open: bool,
    rules_generation: u64,
    history_generation: u64,
    catalog_generation: u64,
    close_focus: FocusHandle,
    _search_events: Subscription,
}

impl EventEmitter<AutomationClosed> for AutomationPanel {}

impl AutomationPanel {
    pub fn new(
        client: EditClient,
        board: Uuid,
        board_name: String,
        owner: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search rules"));
        let search_events = cx.subscribe(&search, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.search_text = input.read(cx).value().to_string();
                cx.notify();
            }
        });
        let mut panel = Self {
            client,
            board,
            board_name: board_name.into(),
            owner,
            tab: AutomationTab::Manage,
            catalog: None,
            catalog_state: LoadState::Idle,
            catalog_error: None,
            rules: None,
            rules_state: LoadState::Idle,
            rules_error: None,
            rules_archived: false,
            rules_has_more: false,
            search,
            search_text: String::new(),
            busy_rule: None,
            expanded: std::collections::HashSet::new(),
            history_rule: None,
            history_runs: Vec::new(),
            history_state: LoadState::Idle,
            history_error: None,
            history_has_more: false,
            history_offset: 0,
            editor: None,
            editor_open: false,
            rules_generation: 0,
            history_generation: 0,
            catalog_generation: 0,
            close_focus: cx.focus_handle().tab_stop(true),
            _search_events: search_events,
        };
        panel.load_catalog(window, cx);
        panel.load_rules(false, window, cx);
        panel
    }

    /// Focus the panel's close control so Escape and Tab behave like Tuesday.
    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        self.close_focus.focus(window, cx);
    }

    // -- loading ----------------------------------------------------------

    fn load_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.catalog_generation += 1;
        let generation = self.catalog_generation;
        self.catalog_state = LoadState::Loading;
        self.catalog_error = None;
        let client = self.client.clone();
        let board = self.board;
        let work = cx
            .background_executor()
            .spawn(async move { client.automation_catalog(board) });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, _window, cx| {
                if panel.catalog_generation != generation {
                    return;
                }
                match result {
                    Ok(catalog) => {
                        panel.catalog = Some(catalog);
                        panel.catalog_state = LoadState::Loaded;
                    }
                    Err(error) => {
                        panel.catalog_state = LoadState::Failed;
                        panel.catalog_error = Some(error_text(&error).to_owned());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_rules(&mut self, more: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.rules_generation += 1;
        let generation = self.rules_generation;
        let offset = if more {
            self.rules.as_ref().map_or(0, |rules| rules.len() as i64)
        } else {
            0
        };
        self.rules_state = LoadState::Loading;
        self.rules_error = None;
        let archived = self.rules_archived;
        let client = self.client.clone();
        let board = self.board;
        let work = cx
            .background_executor()
            .spawn(async move { client.list_automation_rules(board, archived, offset) });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, _window, cx| {
                if panel.rules_generation != generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        panel.rules_has_more = page.len() as i64 == RULE_PAGE;
                        let rules = panel.rules.get_or_insert_with(Vec::new);
                        if more {
                            rules.extend(page);
                        } else {
                            *rules = page;
                        }
                        panel.rules_state = LoadState::Loaded;
                    }
                    Err(error) => {
                        panel.rules_state = LoadState::Failed;
                        panel.rules_error = Some(error_text(&error).to_owned());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_history(&mut self, more: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.history_generation += 1;
        let generation = self.history_generation;
        let offset = if more { self.history_offset } else { 0 };
        self.history_state = LoadState::Loading;
        self.history_error = None;
        let client = self.client.clone();
        let board = self.board;
        let rule = self.history_rule.clone();
        let work = cx.background_executor().spawn(async move {
            client.automation_history(board, rule.as_deref(), offset)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, window, cx| {
                if panel.history_generation != generation {
                    return;
                }
                match result {
                    Ok(mut history) => {
                        panel.history_has_more = history.has_more;
                        panel.history_offset = offset + history.runs.len() as i64;
                        if more {
                            panel.history_runs.append(&mut history.runs);
                        } else {
                            panel.history_runs = history.runs;
                        }
                        panel.history_state = LoadState::Loaded;
                    }
                    Err(error) => {
                        panel.history_state = LoadState::Failed;
                        panel.history_error = Some(error_text(&error).to_owned());
                    }
                }
                let _ = window;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn reload_latest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_catalog(window, cx);
        if self.tab == AutomationTab::Manage {
            self.load_rules(false, window, cx);
        } else {
            self.load_rules(false, window, cx);
            self.load_history(false, window, cx);
        }
    }

    fn set_tab(&mut self, tab: AutomationTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        if tab == AutomationTab::RunHistory {
            if self.rules.is_none() {
                self.load_rules(false, window, cx);
            }
            self.load_history(false, window, cx);
        }
        cx.notify();
    }

    // -- rule actions -----------------------------------------------------

    fn toggle_rule(
        &mut self,
        id: String,
        expected_revision: i64,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy_rule.is_some() {
            return;
        }
        self.busy_rule = Some(id.clone());
        self.rules_error = None;
        let client = self.client.clone();
        let board = self.board;
        let work = cx.background_executor().spawn(async move {
            client.set_automation_rule_enabled(board, &id, expected_revision, enabled)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, _window, cx| {
                panel.busy_rule = None;
                match result {
                    Ok(rule) => {
                        if let Some(rules) = &mut panel.rules
                            && let Some(slot) = rules.iter_mut().find(|entry| entry.id == rule.id)
                        {
                            *slot = rule;
                        }
                    }
                    Err(error) => panel.rules_error = Some(error_text(&error).to_owned()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn archive_rule(
        &mut self,
        id: String,
        expected_revision: i64,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy_rule.is_some() {
            return;
        }
        self.busy_rule = Some(id.clone());
        self.rules_error = None;
        let client = self.client.clone();
        let board = self.board;
        let work = cx.background_executor().spawn(async move {
            client.set_automation_rule_archived(board, &id, expected_revision, archived)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, window, cx| {
                panel.busy_rule = None;
                match result {
                    Ok(_) => panel.load_rules(false, window, cx),
                    Err(error) => {
                        panel.rules_error = Some(error_text(&error).to_owned());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }

    // -- editor -----------------------------------------------------------

    fn open_new_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_open = true;
        if let Some(editor) = &self.editor
            && editor.seed.expected_revision == 0
        {
            cx.notify();
            return;
        }
        let seed = EditorSeed {
            rule_id: format!("custom-{}", Uuid::new_v4().simple()),
            expected_revision: 0,
            name: "New automation".to_owned(),
            enabled: true,
            trigger_chosen: false,
            definition: AutomationDefinition {
                trigger: AutomationTrigger {
                    kind: AutomationTriggerKind::ItemCreated,
                    column_id: None,
                    value: None,
                    previous_value: None,
                },
                conditions: Vec::new(),
                actions: Vec::new(),
                conditions_mode: AutomationConditionMode::All,
                action_groups: Vec::new(),
            },
        };
        self.editor = Some(Editor::new(seed, window, cx));
        cx.notify();
    }

    fn open_rule_editor(&mut self, rule: &AutomationRule, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_open = true;
        if let Some(editor) = &self.editor
            && editor.seed.rule_id == rule.id
        {
            cx.notify();
            return;
        }
        self.editor = Some(Editor::new(seed_from_rule(rule), window, cx));
        cx.notify();
    }

    fn duplicate_rule(&mut self, rule: &AutomationRule, window: &mut Window, cx: &mut Context<Self>) {
        let mut seed = seed_from_rule(rule);
        seed.rule_id = format!("custom-{}", Uuid::new_v4().simple());
        seed.expected_revision = 0;
        seed.name = format!("{} (copy)", rule.name);
        seed.enabled = false;
        self.editor = Some(Editor::new(seed, window, cx));
        self.editor_open = true;
        cx.notify();
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(|editor| editor.saving) {
            return;
        }
        // Keep the draft; Back only hides the editor.
        if let Some(editor) = &mut self.editor {
            editor.composer = None;
        }
        self.editor_open = false;
        cx.notify();
    }

    fn discard_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let seed = editor.seed.clone();
        self.editor = Some(Editor::new(seed, window, cx));
        cx.notify();
    }

    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        if editor.saving || !editor.trigger_chosen || editor.total_actions() == 0 {
            return;
        }
        let catalog = self.catalog.clone();
        let name = editor.name.read(cx).value().trim().to_owned();
        let input = SaveAutomationRule {
            id: editor.seed.rule_id.clone(),
            expected_revision: editor.seed.expected_revision,
            name: name.clone(),
            enabled: editor.enabled,
            definition: editor.definition(catalog.as_ref(), cx),
        };
        if name.is_empty() {
            if let Some(editor) = &mut self.editor {
                editor.error = Some("Enter an automation name.".to_owned());
            }
            cx.notify();
            return;
        }
        if let Some(editor) = &mut self.editor {
            editor.saving = true;
            editor.error = None;
            editor.composer = None;
        }
        let client = self.client.clone();
        let board = self.board;
        let work = cx
            .background_executor()
            .spawn(async move { client.save_automation_rule(board, &input) });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |panel, window, cx| {
                match result {
                    Ok(saved) => {
                        panel.editor = None;
                        panel.editor_open = false;
                        if let Some(rules) = &mut panel.rules
                            && let Some(slot) = rules.iter_mut().find(|entry| entry.id == saved.id)
                        {
                            *slot = saved;
                        }
                        panel.load_rules(false, window, cx);
                    }
                    Err(error) => {
                        if let Some(editor) = &mut panel.editor {
                            editor.saving = false;
                            editor.error = Some(error_text(&error).to_owned());
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }

    // -- editor mutations -------------------------------------------------

    fn with_editor<R>(&mut self, update: impl FnOnce(&mut Editor) -> R) -> Option<R> {
        self.editor.as_mut().map(update)
    }

    fn set_trigger_kind(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = trigger_kind_from(value) else {
            return;
        };
        let value_input = input_for(window, cx, "");
        let previous_input = input_for(window, cx, "");
        self.with_editor(|editor| {
            editor.trigger_chosen = true;
            editor.trigger = TriggerDraft {
                kind,
                column_id: None,
                value: None,
                previous_value: None,
                value_input,
                previous_input,
            };
            editor.error = None;
        });
        cx.notify();
    }

    fn set_trigger_column(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(trigger) = self.editor.as_ref().map(|editor| editor.trigger.value_input.clone())
        else {
            return;
        };
        let previous = self
            .editor
            .as_ref()
            .map(|editor| editor.trigger.previous_input.clone());
        trigger.update(cx, |input, cx| input.set_value("", window, cx));
        if let Some(previous) = previous {
            previous.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.with_editor(|editor| {
            editor.trigger.column_id = Some(value);
            editor.trigger.value = None;
            editor.trigger.previous_value = None;
        });
        cx.notify();
    }

    fn set_trigger_value(&mut self, previous: bool, value: Option<String>, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if previous {
                editor.trigger.previous_value = value;
            } else {
                editor.trigger.value = value;
            }
        });
        cx.notify();
    }

    fn clear_trigger(&mut self, cx: &mut Context<Self>) {
        self.with_editor(|editor| editor.trigger_chosen = false);
        cx.notify();
    }

    fn set_conditions_mode(&mut self, mode: AutomationConditionMode, cx: &mut Context<Self>) {
        self.with_editor(|editor| editor.conditions_mode = mode);
        cx.notify();
    }

    fn set_group_mode(&mut self, group: usize, mode: AutomationConditionMode, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if let Some(group) = editor.groups.get_mut(group) {
                group.mode = mode;
            }
        });
        cx.notify();
    }

    fn add_condition(
        &mut self,
        group: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let column = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.columns.first())
            .map(|column| column.id.clone());
        let input = input_for(window, cx, "");
        self.with_editor(|editor| {
            if let Some(conditions) = editor.conditions_mut(group) {
                conditions.push(ConditionDraft {
                    column_id: column,
                    operator: AutomationConditionOperator::Is,
                    value: None,
                    input,
                });
            }
        });
        cx.notify();
    }

    fn remove_condition(&mut self, group: Option<usize>, index: usize, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if let Some(conditions) = editor.conditions_mut(group) {
                if index < conditions.len() {
                    conditions.remove(index);
                }
            }
        });
        cx.notify();
    }

    fn set_condition_column(
        &mut self,
        group: Option<usize>,
        index: usize,
        column: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(condition) = editor.condition_mut(group, index) {
                condition.column_id = Some(column);
                condition.value = None;
                condition.input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            }
        });
        cx.notify();
    }

    fn set_condition_operator(
        &mut self,
        group: Option<usize>,
        index: usize,
        operator: AutomationConditionOperator,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(condition) = editor.condition_mut(group, index) {
                condition.operator = operator;
            }
        });
        cx.notify();
    }

    fn set_condition_choice(
        &mut self,
        group: Option<usize>,
        index: usize,
        value: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(condition) = editor.condition_mut(group, index) {
                condition.value = value;
            }
        });
        cx.notify();
    }

    fn add_action(
        &mut self,
        group: Option<usize>,
        kind: AutomationActionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = new_action(kind, window, cx);
        self.with_editor(|editor| {
            if let Some(actions) = editor.actions_mut(group) {
                actions.push(draft);
            }
        });
        cx.notify();
    }

    fn remove_action(&mut self, group: Option<usize>, index: usize, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if let Some(actions) = editor.actions_mut(group)
                && index < actions.len()
            {
                actions.remove(index);
            }
        });
        cx.notify();
    }

    fn set_action_kind(
        &mut self,
        group: Option<usize>,
        index: usize,
        kind: AutomationActionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = new_action(kind, window, cx);
        self.with_editor(|editor| {
            if let Some(actions) = editor.actions_mut(group)
                && index < actions.len()
            {
                actions[index] = draft;
            }
        });
        cx.notify();
    }

    fn set_action_column(
        &mut self,
        group: Option<usize>,
        index: usize,
        column: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(action) = editor.action_mut(group, index) {
                action.column_id = Some(column);
                action.value = None;
                action.input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            }
        });
        cx.notify();
    }

    fn set_action_choice(
        &mut self,
        group: Option<usize>,
        index: usize,
        value: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(action) = editor.action_mut(group, index) {
                action.value = value;
            }
        });
        cx.notify();
    }

    fn set_action_people(
        &mut self,
        group: Option<usize>,
        index: usize,
        people: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(action) = editor.action_mut(group, index) {
                action.person_ids = people;
            }
        });
        cx.notify();
    }

    fn set_action_recipient(
        &mut self,
        group: Option<usize>,
        index: usize,
        recipient: AutomationRecipient,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(action) = editor.action_mut(group, index) {
                action.recipient = recipient;
            }
        });
        cx.notify();
    }

    fn set_action_date_basis(
        &mut self,
        group: Option<usize>,
        index: usize,
        basis: AutomationDateBasis,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(action) = editor.action_mut(group, index) {
                action.date_basis = basis;
            }
        });
        cx.notify();
    }

    fn move_action_up(
        &mut self,
        group: Option<usize>,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        self.with_editor(|editor| {
            if let Some(actions) = editor.actions_mut(group)
                && index > 0
                && index < actions.len()
            {
                actions.swap(index - 1, index);
            }
        });
        cx.notify();
    }

    fn add_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let column = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.columns.first())
            .map(|column| column.id.clone());
        let input = input_for(window, cx, "");
        self.with_editor(|editor| {
            editor.groups.push(GroupDraft {
                mode: AutomationConditionMode::All,
                conditions: vec![ConditionDraft {
                    column_id: column,
                    operator: AutomationConditionOperator::Is,
                    value: None,
                    input,
                }],
                actions: Vec::new(),
            });
        });
        cx.notify();
    }

    fn remove_group(&mut self, index: usize, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if index < editor.groups.len() {
                editor.groups.remove(index);
            }
        });
        cx.notify();
    }

    fn move_group_up(&mut self, index: usize, cx: &mut Context<Self>) {
        self.with_editor(|editor| {
            if index > 0 && index < editor.groups.len() {
                editor.groups.swap(index - 1, index);
            }
        });
        cx.notify();
    }

    fn open_composer(
        &mut self,
        group: Option<usize>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(action) = editor.action(group, index) else {
            return;
        };
        let title = input_for(window, cx, action.title.as_deref().unwrap_or(""));
        let message = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(6)
                .placeholder("Write a message and add item or board details")
                .default_value(action.message.as_deref().unwrap_or(""))
        });
        if let Some(editor) = &mut self.editor {
            editor.composer = Some(Composer {
                group,
                index,
                title,
                message,
            });
        }
        cx.notify();
    }

    fn append_token(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.editor.as_ref().and_then(|editor| editor.composer.as_ref()) else {
            return;
        };
        let message = composer.message.clone();
        message.update(cx, |state, cx| {
            let mut text = state.value().to_string();
            text.push_str(token);
            state.set_value(text, window, cx);
        });
        cx.notify();
    }

    fn finish_composer(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(composer) = editor.composer.take() else {
            return;
        };
        let title = composer.title.read(cx).value().to_string();
        let message = composer.message.read(cx).value().to_string();
        if let Some(action) = editor.action_mut(composer.group, composer.index) {
            action.title = text_value(&title);
            action.message = text_value(&message);
        }
        cx.notify();
    }
}

fn seed_from_rule(rule: &AutomationRule) -> EditorSeed {
    EditorSeed {
        rule_id: rule.id.clone(),
        expected_revision: rule.revision,
        name: rule.name.clone(),
        enabled: rule.enabled,
        trigger_chosen: true,
        definition: rule.definition.clone().unwrap_or(AutomationDefinition {
            trigger: AutomationTrigger {
                kind: AutomationTriggerKind::ItemCreated,
                column_id: None,
                value: None,
                previous_value: None,
            },
            conditions: Vec::new(),
            actions: Vec::new(),
            conditions_mode: AutomationConditionMode::All,
            action_groups: Vec::new(),
        }),
    }
}

fn new_action(
    kind: AutomationActionKind,
    window: &mut Window,
    cx: &mut Context<AutomationPanel>,
) -> ActionDraft {
    ActionDraft {
        kind,
        column_id: (kind == AutomationActionKind::AssignCreator).then(|| "Entered".to_owned()),
        value: None,
        person_ids: Vec::new(),
        days_input: input_for(window, cx, "0"),
        date_basis: AutomationDateBasis::Today,
        recipient: AutomationRecipient::People,
        title: None,
        message: None,
        input: input_for(window, cx, ""),
    }
}


// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl AutomationPanel {
    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(AutomationClosed);
    }

    fn toggle_expanded(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.expanded.insert(id.to_owned()) {
            self.expanded.remove(id);
        }
        cx.notify();
    }

    fn choose_archive_view(
        &mut self,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rules_archived == archived {
            return;
        }
        self.rules_archived = archived;
        self.rules = None;
        self.load_rules(false, window, cx);
    }

    fn choose_history_rule(
        &mut self,
        rule: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.history_rule == rule {
            return;
        }
        self.history_rule = rule;
        self.history_runs.clear();
        self.history_offset = 0;
        self.load_history(false, window, cx);
    }

    fn render_manage(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let search = self.search.clone();
        let _ = search;
        let mut body = div()
            .id("automation-manage")
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .overflow_y_scroll();
        let mut tools = div().flex().items_center().gap_2();
        if self.owner {
            tools = tools.child(
                Button::new("create-automation")
                    .small()
                    .label("Create automation")
                    .on_click(cx.listener(|panel, _, w, cx| panel.open_new_editor(w, cx))),
            );
        }
        tools = tools
            .child(Input::new(&self.search).id("automation-search").small())
            .child(
                Label::new(match &self.rules {
                    Some(rules) => format!(
                        "{} enabled � {} rules",
                        rules.iter().filter(|rule| rule.enabled).count(),
                        rules.len()
                    ),
                    None => String::new(),
                })
                .text_sm(),
            )
            .child(
                Button::new("reload-automation-rules")
                    .small()
                    .label("Reload rules")
                    .disabled(matches!(self.rules_state, LoadState::Loading) || self.busy_rule.is_some())
                    .on_click(cx.listener(|panel, _, w, cx| panel.load_rules(false, w, cx))),
            );
        body = body.child(tools);
        let mut list_tabs = div().flex().items_center().gap_2();
        list_tabs = list_tabs
            .child(
                Button::new("rules-current")
                    .small()
                    .label("Board automations")
                    .selected(!self.rules_archived)
                    .on_click(cx.listener(|panel, _, w, cx| panel.choose_archive_view(false, w, cx))),
            )
            .child(
                Button::new("rules-archived")
                    .small()
                    .label("Archived")
                    .selected(self.rules_archived)
                    .on_click(cx.listener(|panel, _, w, cx| panel.choose_archive_view(true, w, cx))),
            );
        body = body.child(list_tabs);
        if !self.owner {
            body = body.child(Label::new("Rules are read-only for this account. An Owner can change configuration.").text_sm());
        }
        if let Some(error) = &self.rules_error {
            body = body.child(
                div()
                    .id("automation-rule-error")
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .p_2()
                    .child(Label::new(error.clone()))
                    .child(
                        Button::new("reload-latest")
                            .small()
                            .label("Reload latest")
                            .on_click(cx.listener(|panel, _, w, cx| panel.reload_latest(w, cx))),
                    ),
            );
        }
        if matches!(self.catalog_state, LoadState::Failed) && self.catalog.is_none() {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(self.catalog_error.clone().unwrap_or_else(|| "Could not load automation choices.".to_owned())))
                    .child(
                        Button::new("retry-automation-catalog")
                            .small()
                            .label("Retry loading choices")
                            .on_click(cx.listener(|panel, _, w, cx| panel.load_catalog(w, cx))),
                    ),
            );
        }
        if matches!(self.rules_state, LoadState::Loading) && self.rules.is_none() {
            body = body.child(Label::new("Loading rules�").text_sm());
        }
        let visible: Vec<AutomationRule> = self
            .rules
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|rule| {
                let needle = self.search_text.trim().to_lowercase();
                needle.is_empty()
                    || format!("{} {} {}", rule.name, rule.trigger, rule.actions.join(" "))
                        .to_lowercase()
                        .contains(&needle)
            })
            .collect();
        if matches!(self.rules_state, LoadState::Loaded) && !self.rules_archived && self.rules.as_ref().is_some_and(Vec::is_empty) {
            body = body.child(Label::new("No automation rules").text_lg());
        }
        if matches!(self.rules_state, LoadState::Loaded) && self.rules.as_ref().is_some_and(|rules| !rules.is_empty()) && visible.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new("No rules match this search."))
                    .child(
                        Button::new("clear-automation-search")
                            .small()
                            .label("Clear search")
                            .on_click(cx.listener(|panel, _, w, cx| {
                                panel.search.update(cx, |input, cx| input.set_value("", w, cx));
                                panel.search_text.clear();
                                cx.notify();
                            })),
                    ),
            );
        }
        for rule in visible {
            body = body.child(self.render_rule(&rule, cx));
        }
        if self.rules_has_more {
            body = body.child(
                Button::new("load-more-automations")
                    .small()
                    .label("Load more automations")
                    .disabled(matches!(self.rules_state, LoadState::Loading))
                    .on_click(cx.listener(|panel, _, w, cx| panel.load_rules(true, w, cx))),
            );
        }
        let _ = weak;
        let _ = window;
        body.into_any_element()
    }

    fn render_rule(&self, rule: &AutomationRule, cx: &mut Context<Self>) -> AnyElement {
        let id = rule.id.clone();
        let ready = rule.disposition == RuleDisposition::Ready
            && rule.definition.is_some()
            && !rule.archived;
        let editable = self.owner && ready;
        let busy = self.busy_rule.as_deref() == Some(rule.id.as_str());
        let expanded = self.expanded.contains(&rule.id);
        let mut card = div()
            .id(SharedString::from(format!("automation-rule-{}", rule.id)))
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .p_3();
        let mut heading = div().flex().items_start().justify_between().gap_2();
        let mut title = div().flex().flex_col().gap_1();
        let sentence = rule_sentence(rule);
        if editable {
            title = title.child(
                Button::new(SharedString::from(format!("automation-sentence-{}", rule.id)))
                    .small()
                    .label(sentence)
                    .on_click(cx.listener({
                        let rule = rule.clone();
                        move |panel, _, w, cx| panel.open_rule_editor(&rule, w, cx)
                    })),
            );
        } else {
            title = title.child(Label::new(sentence).text_sm());
        }
        title = title.child(Label::new(rule.name.clone()).text_xs());
        if let Some(issue) = &rule.dependency_issue {
            title = title.child(Label::new(issue.clone()).text_xs());
        }
        let mut badges = div().flex().items_center().gap_2();
        badges = badges
            .child(
                Label::new(if rule.enabled { "Enabled" } else { "Disabled" })
                    .text_xs()
                    .px_2()
                    .rounded_full()
                    .bg(if rule.enabled { cx.theme().selection } else { cx.theme().muted }),
            )
            .child(Label::new(disposition_label(rule.disposition)).text_xs())
            .child(Label::new(format!("Revision {}", rule.revision)).text_xs());
        title = title.child(badges);
        heading = heading.child(title);
        heading = heading.child(
            Button::new(SharedString::from(format!("automation-toggle-{}", rule.id)))
                .small()
                .label(if busy {
                    "Saving�".to_owned()
                } else if rule.enabled {
                    "On".to_owned()
                } else {
                    "Off".to_owned()
                })
                .disabled(!self.owner || busy || rule.disposition != RuleDisposition::Ready || rule.archived)
                .on_click(cx.listener({
                    let id = id.clone();
                    let revision = rule.revision;
                    let enabled = !rule.enabled;
                    move |panel, _, w, cx| panel.toggle_rule(id.clone(), revision, enabled, w, cx)
                })),
        );
        card = card.child(heading);
        card = card.child(
            Button::new(SharedString::from(format!("automation-details-{}", rule.id)))
                .small()
                .label(if expanded { "Hide recipe details" } else { "Recipe details" })
                .on_click(cx.listener({
                    let id = id.clone();
                    move |panel, _, _, cx| panel.toggle_expanded(&id, cx)
                })),
        );
        if expanded {
            let mut details = div().flex().flex_col().gap_1().text_sm();
            details = details
                .child(Label::new(format!("When: {}", rule.trigger)))
                .child(Label::new(format!(
                    "If: {}",
                    if rule.conditions.is_empty() { "always".to_owned() } else { rule.conditions.join(" and ") }
                )))
                .child(Label::new(format!("Then: {}", rule.actions.join(", then "))));
            if let Some(definition) = &rule.definition
                && !definition.action_groups.is_empty()
            {
                details = details.child(Label::new(format!(
                    "{} conditional action groups run after the common actions.",
                    definition.action_groups.len()
                )));
            }
            if let Some(canonical) = &rule.canonical_rule_id {
                details = details.child(Label::new(format!("Canonical rule: {canonical}")));
            }
            if !rule.decision.is_empty() {
                details = details.child(Label::new(rule.decision.clone()).text_xs());
            }
            if !rule.evidence.is_empty() {
                details = details.child(Label::new(rule.evidence.join(" � ")).text_xs());
            }
            card = card.child(details);
        }
        let mut footer = div().flex().items_center().gap_2();
        if editable {
            footer = footer.child(
                Button::new(SharedString::from(format!("automation-edit-{}", rule.id)))
                    .small()
                    .label("Edit automation")
                    .on_click(cx.listener({
                        let rule = rule.clone();
                        move |panel, _, w, cx| panel.open_rule_editor(&rule, w, cx)
                    })),
            );
        }
        if self.owner && rule.definition.is_some() && !rule.archived && rule.disposition == RuleDisposition::Ready {
            footer = footer.child(
                Button::new(SharedString::from(format!("automation-duplicate-{}", rule.id)))
                    .small()
                    .label("Duplicate")
                    .on_click(cx.listener({
                        let rule = rule.clone();
                        move |panel, _, w, cx| panel.duplicate_rule(&rule, w, cx)
                    })),
            );
        }
        if self.owner {
            footer = footer.child(
                Button::new(SharedString::from(format!("automation-archive-{}", rule.id)))
                    .small()
                    .label(if rule.archived { "Restore" } else { "Archive" })
                    .disabled(busy)
                    .on_click(cx.listener({
                        let id = id.clone();
                        let revision = rule.revision;
                        let archived = !rule.archived;
                        move |panel, _, w, cx| panel.archive_rule(id.clone(), revision, archived, w, cx)
                    })),
            );
        }
        footer = footer.child(
            Button::new(SharedString::from(format!("automation-history-{}", rule.id)))
                .small()
                .label("View runs")
                .on_click(cx.listener({
                    let id = id.clone();
                    move |panel, _, w, cx| {
                        panel.set_tab(AutomationTab::RunHistory, w, cx);
                        panel.choose_history_rule(Some(id.clone()), w, cx);
                    }
                })),
        );
        card = card.child(footer);
        card.into_any_element()
    }

    fn render_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div()
            .id("automation-history")
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .overflow_y_scroll();
        let rules = self.rules.clone().unwrap_or_default();
        let current = self
            .history_rule
            .as_ref()
            .and_then(|id| rules.iter().find(|rule| &rule.id == id))
            .map(|rule| rule.name.clone())
            .unwrap_or_else(|| "All rules".to_owned());
        let weak = cx.weak_entity();
        let rules_for_menu = rules.clone();
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("history-rule-filter")
                        .small()
                        .label(current)
                        .dropdown_menu({
                            let weak = weak.clone();
                            move |mut menu, _, _| {
                                let weak_all = weak.clone();
                                menu = menu.item(PopupMenuItem::new("All rules").on_click(
                                    move |_, w, cx| {
                                        let _ = weak_all.update(cx, |panel, cx| {
                                            panel.choose_history_rule(None, w, cx)
                                        });
                                    },
                                ));
                                for rule in &rules_for_menu {
                                    let id = rule.id.clone();
                                    let weak_rule = weak.clone();
                                    menu = menu.item(PopupMenuItem::new(rule.name.clone()).on_click(
                                        move |_, w, cx| {
                                            let _ = weak_rule.update(cx, |panel, cx| {
                                                panel.choose_history_rule(Some(id.clone()), w, cx)
                                            });
                                        },
                                    ));
                                }
                                menu
                            }
                        }),
                )
                .child(
                    Button::new("reload-automation-history")
                        .small()
                        .label("Reload history")
                        .disabled(matches!(self.history_state, LoadState::Loading))
                        .on_click(cx.listener(|panel, _, w, cx| panel.load_history(false, w, cx))),
                ),
        );
        if let Some(error) = &self.history_error {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .p_2()
                    .child(Label::new(error.clone()))
                    .child(
                        Button::new("reload-history-latest")
                            .small()
                            .label("Reload latest")
                            .on_click(cx.listener(|panel, _, w, cx| panel.load_history(false, w, cx))),
                    ),
            );
        }
        if matches!(self.history_state, LoadState::Loading) && self.history_runs.is_empty() {
            body = body.child(Label::new("Loading run history�").text_sm());
        }
        if matches!(self.history_state, LoadState::Loaded) && self.history_runs.is_empty() {
            body = body.child(Label::new(if self.history_rule.is_some() {
                "No runs for this rule yet"
            } else {
                "No automation runs yet"
            }).text_lg());
        }
        for run in &self.history_runs {
            let mut card = div()
                .id(SharedString::from(format!("automation-run-{}", run.id)))
                .flex()
                .flex_col()
                .gap_1()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .p_3();
            card = card
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(Label::new(run.rule_name.clone()))
                        .child(
                            Label::new(outcome_label(run.outcome))
                                .text_xs(),
                        ),
                )
                .child(Label::new(format!(
                    "{} � {} � rule revision {}",
                    format_timestamp(&run.created_at),
                    person_name(self.catalog.as_ref(), &run.actor_id),
                    run.rule_revision
                )).text_xs())
                .child(Label::new(format!(
                    "Item: {}",
                    item_name(self.catalog.as_ref(), &run.item_id)
                )).text_sm())
                .child(Label::new(format!(
                    "Trigger: {}",
                    snapshot_text(&run.trigger_snapshot, "Trigger", "Recorded item transition").join(" � ")
                )).text_xs())
                .child(Label::new(if run.outcome == boardcraft_mobile_edit_client::AutomationOutcome::Succeeded {
                    format!("Actions: {}", snapshot_text(&run.action_snapshot, "Actions", "Recorded local action").join(" � "))
                } else {
                    "Actions: No actions applied.".to_owned()
                }).text_xs())
                .child(Label::new(format!(
                    "Run {} � item revision {} � rule {}",
                    run.id, run.item_revision, run.rule_id
                )).text_xs());
            body = body.child(card);
        }
        if self.history_has_more {
            body = body.child(
                Button::new("load-more-history")
                    .small()
                    .label("Load more runs")
                    .disabled(matches!(self.history_state, LoadState::Loading))
                    .on_click(cx.listener(|panel, _, w, cx| panel.load_history(true, w, cx))),
            );
        }
        body.into_any_element()
    }
}

impl Render for AutomationPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlay = div()
            .id("automation-panel-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.92))
            .track_focus(&self.close_focus)
            .on_key_down(cx.listener(|panel, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    if panel.editor_open {
                        panel.close_editor(cx);
                    } else {
                        panel.close(cx);
                    }
                    cx.stop_propagation();
                }
            }));
        let mut panel = div()
            .id("automation-panel")
            .w(gpui_kit::px(960.))
            .h(gpui_kit::px(760.))
            .max_w_full()
            .max_h_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .overflow_hidden();
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .p_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(Label::new("Automations").text_xl())
                        .child(Label::new(self.board_name.to_string()).text_sm()),
                )
                .child(
                    Button::new("close-automation-panel")
                        .small()
                        .label("Close")
                        .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                ),
        );
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .child(
                    Button::new("automation-tab-manage")
                        .small()
                        .label("Manage")
                        .selected(self.tab == AutomationTab::Manage)
                        .on_click(cx.listener(|panel, _, w, cx| panel.set_tab(AutomationTab::Manage, w, cx))),
                )
                .child(
                    Button::new("automation-tab-history")
                        .small()
                        .label("Run history")
                        .selected(self.tab == AutomationTab::RunHistory)
                        .on_click(cx.listener(|panel, _, w, cx| panel.set_tab(AutomationTab::RunHistory, w, cx))),
                ),
        );
        panel = panel.child(match self.tab {
            AutomationTab::Manage => self.render_manage(_window, cx),
            AutomationTab::RunHistory => self.render_history(cx),
        });
        let mut root = overlay.child(panel);
        if self.editor_open {
            root = root.child(self.render_editor(_window, cx));
        }
        root
    }
}

fn wire<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn picker(
    id: impl Into<gpui_kit::ElementId>,
    label: String,
    choices: Vec<AutomationChoice>,
    on_pick: Rc<dyn Fn(String, &mut Window, &mut App)>,
) -> AnyElement {
    Button::new(id)
        .small()
        .label(label)
        .dropdown_menu(move |mut menu, _, _| {
            for choice in choices.iter() {
                let value = choice.value.clone();
                let on_pick = on_pick.clone();
                menu = menu.item(
                    PopupMenuItem::new(choice.name.clone())
                        .on_click(move |_, window, cx| on_pick(value.clone(), window, cx)),
                );
            }
            menu
        })
        .into_any_element()
}

impl AutomationPanel {
    fn render_editor(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        let catalog = self.catalog.clone();
        let weak = cx.weak_entity();
        let mut layer = div()
            .id("automation-editor-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.92));
        let mut sheet = div()
            .id("automation-editor")
            .w(gpui_kit::px(900.))
            .h(gpui_kit::px(740.))
            .max_w_full()
            .max_h_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .overflow_hidden();
        sheet = sheet.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .p_3()
                .child(
                    Button::new("automation-editor-back")
                        .small()
                        .label("Back")
                        .disabled(editor.saving)
                        .on_click(cx.listener(|panel, _, _, cx| panel.close_editor(cx))),
                )
                .child(Label::new(self.board_name.to_string()).text_sm())
                .child(
                    Button::new("automation-editor-discard")
                        .small()
                        .label("Discard draft")
                        .disabled(editor.saving)
                        .on_click(cx.listener(|panel, _, w, cx| panel.discard_draft(w, cx))),
                )
                .child(
                    Button::new("automation-editor-close")
                        .small()
                        .label("Close")
                        .disabled(editor.saving)
                        .on_click(cx.listener(|panel, _, _, cx| panel.close_editor(cx))),
                ),
        );
        let mut body = div()
            .id("automation-editor-body")
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .overflow_y_scroll();
        body = body
            .child(
                Label::new(if editor.is_existing() {
                    "Edit automation"
                } else {
                    "Create automation"
                })
                .text_xl(),
            )
            .child(
                Input::new(&editor.name)
                    .id("automation-name")
                    .disabled(editor.saving),
            );
        if let Some(error) = &editor.error {
            body = body.child(
                div()
                    .id("automation-editor-error")
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .p_2()
                    .child(Label::new(error.clone())),
            );
        }
        let trigger_kind_choices = catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .triggers
                    .iter()
                    .map(|kind| choice(wire(kind), trigger_label(*kind)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let trigger_label_text = if editor.trigger_chosen {
            trigger_label(editor.trigger.kind)
        } else {
            "this happens"
        };
        let weak_trigger = weak.clone();
        let mut trigger = div().flex().items_center().flex_wrap().gap_2();
        trigger = trigger
            .child(Label::new("When"))
            .child(picker(
                "automation-trigger-kind",
                trigger_label_text.to_owned(),
                trigger_kind_choices,
                Rc::new(move |value, window, cx| {
                    let _ = weak_trigger.update(cx, |panel, cx| {
                        panel.set_trigger_kind(&value, window, cx)
                    });
                }),
            ));
        if editor.trigger_chosen {
            match editor.trigger.kind {
                AutomationTriggerKind::ButtonClicked => {
                    let choices = named_columns(catalog.as_ref(), "button");
                    let handle = weak.clone();
                    trigger = trigger.child(picker(
                        "automation-trigger-button",
                        editor
                            .trigger
                            .column_id
                            .as_deref()
                            .and_then(|id| choices.iter().find(|choice| choice.value == id))
                            .map(|choice| choice.name.clone())
                            .unwrap_or_else(|| "button column".to_owned()),
                        choices,
                        Rc::new(move |value, window, cx| {
                            let _ = handle.update(cx, |panel, cx| {
                                panel.set_trigger_column(value, window, cx)
                            });
                        }),
                    ));
                }
                AutomationTriggerKind::PersonAssigned => {
                    let choices = named_columns(catalog.as_ref(), "people");
                    let handle = weak.clone();
                    trigger = trigger.child(picker(
                        "automation-trigger-people",
                        editor
                            .trigger
                            .column_id
                            .as_deref()
                            .and_then(|id| choices.iter().find(|choice| choice.value == id))
                            .map(|choice| choice.name.clone())
                            .unwrap_or_else(|| "people column".to_owned()),
                        choices,
                        Rc::new(move |value, window, cx| {
                            let _ = handle.update(cx, |panel, cx| {
                                panel.set_trigger_column(value, window, cx)
                            });
                        }),
                    ));
                }
                AutomationTriggerKind::ColumnChanged => {
                    let columns = column_choices(catalog.as_ref());
                    let handle = weak.clone();
                    trigger = trigger.child(picker(
                        "automation-trigger-column",
                        editor
                            .trigger
                            .column_id
                            .as_deref()
                            .and_then(|id| columns.iter().find(|choice| choice.value == id))
                            .map(|choice| choice.name.clone())
                            .unwrap_or_else(|| "column".to_owned()),
                        columns,
                        Rc::new(move |value, window, cx| {
                            let _ = handle.update(cx, |panel, cx| {
                                panel.set_trigger_column(value, window, cx)
                            });
                        }),
                    ));
                    let kind =
                        column_choice_kind(catalog.as_ref(), editor.trigger.column_id.as_deref());
                    if is_text_kind(kind) {
                        trigger = trigger
                            .child(Label::new("from"))
                            .child(
                                Input::new(&editor.trigger.previous_input)
                                    .id("trigger-previous")
                                    .small(),
                            )
                            .child(Label::new("to"))
                            .child(
                                Input::new(&editor.trigger.value_input)
                                    .id("trigger-value")
                                    .small(),
                            );
                    } else {
                        let choices = trigger_value_choices(
                            catalog.as_ref(),
                            editor.trigger.column_id.as_deref(),
                        );
                        let handle = weak.clone();
                        trigger = trigger.child(Label::new("from")).child(picker(
                            "trigger-previous-choice",
                            choice_name(&choices, editor.trigger.previous_value.as_deref()),
                            choices.clone(),
                            Rc::new(move |value, _, cx| {
                                let _ = handle.update(cx, |panel, cx| {
                                    panel.set_trigger_value(true, text_value(&value), cx)
                                });
                            }),
                        ));
                        let handle = weak.clone();
                        trigger = trigger.child(Label::new("to")).child(picker(
                            "trigger-value-choice",
                            choice_name(&choices, editor.trigger.value.as_deref()),
                            choices,
                            Rc::new(move |value, _, cx| {
                                let _ = handle.update(cx, |panel, cx| {
                                    panel.set_trigger_value(false, text_value(&value), cx)
                                });
                            }),
                        ));
                    }
                }
                _ => {}
            }
            body = body.child(trigger);
        } else {
            body = body.child(trigger);
        }
        let columns = column_choices(catalog.as_ref());
        let maximum_conditions = catalog
            .as_ref()
            .map_or(8, |catalog| catalog.maximum_conditions as usize);
        let maximum_actions = catalog
            .as_ref()
            .map_or(8, |catalog| catalog.maximum_actions as usize);
        let total_conditions = editor.total_conditions();
        let total_actions = editor.total_actions();
        let mut conditions = div().flex().flex_col().gap_2();
        if !editor.conditions.is_empty() {
            let handle = weak.clone();
            conditions = conditions.child(picker(
                "automation-conditions-mode",
                mode_label(editor.conditions_mode).to_owned(),
                vec![
                    choice("all", "all conditions match (AND)"),
                    choice("any", "any condition matches (OR)"),
                ],
                Rc::new(move |value, _, cx| {
                    if let Some(mode) = mode_from(&value) {
                        let _ = handle.update(cx, |panel, cx| panel.set_conditions_mode(mode, cx));
                    }
                }),
            ));
        }
        for (index, condition) in editor.conditions.iter().enumerate() {
            conditions = conditions.child(self.render_condition(
                None,
                index,
                condition,
                &columns,
                catalog.as_ref(),
                cx,
            ));
        }
        conditions = conditions.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("add-automation-condition")
                        .small()
                        .label("Add condition")
                        .disabled(total_conditions >= maximum_conditions)
                        .on_click(cx.listener(|panel, _, w, cx| panel.add_condition(None, w, cx))),
                )
                .child(
                    Button::new("clear-automation-trigger")
                        .small()
                        .label("Clear trigger")
                        .on_click(cx.listener(|panel, _, _, cx| panel.clear_trigger(cx))),
                ),
        );
        body = body.child(conditions);
        for (index, action) in editor.actions.iter().enumerate() {
            body = body.child(self.render_action(
                None,
                index,
                action,
                catalog.as_ref(),
                editor.trigger.kind,
                maximum_actions,
                cx,
            ));
        }
        let action_choices = catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .actions
                    .iter()
                    .map(|kind| choice(wire(kind), action_label(*kind)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let handle = weak.clone();
        body = body.child(picker(
            "add-automation-action",
            if editor.actions.is_empty() {
                "Then: choose an action".to_owned()
            } else {
                "Add another action".to_owned()
            },
            action_choices.clone(),
            Rc::new(move |value, window, cx| {
                if let Some(kind) = action_kind_from(&value) {
                    let _ = handle.update(cx, |panel, cx| panel.add_action(None, kind, window, cx));
                }
            }),
        ));
        for (group_index, group) in editor.groups.iter().enumerate() {
            let mut group_box = div()
                .id(SharedString::from(format!(
                    "automation-group-{group_index}"
                )))
                .flex()
                .flex_col()
                .gap_2()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .p_2();
            let mut heading = div()
                .flex()
                .items_center()
                .gap_2()
                .child(Label::new(format!("If group {}", group_index + 1)));
            let weak_group = weak.clone();
            heading = heading.child(picker(
                SharedString::from(format!("group-mode-{group_index}")),
                mode_label(group.mode).to_owned(),
                vec![
                    choice("all", "all conditions match (AND)"),
                    choice("any", "any condition matches (OR)"),
                ],
                Rc::new(move |value, _, cx| {
                    if let Some(mode) = mode_from(&value) {
                        let _ = weak_group
                            .update(cx, |panel, cx| panel.set_group_mode(group_index, mode, cx));
                    }
                }),
            ));
            if group_index > 0 {
                heading = heading.child(
                    Button::new(SharedString::from(format!("group-up-{group_index}")))
                        .small()
                        .label("Move group up")
                        .on_click(
                            cx.listener(move |panel, _, _, cx| panel.move_group_up(group_index, cx)),
                        ),
                );
            }
            heading = heading.child(
                Button::new(SharedString::from(format!("group-remove-{group_index}")))
                    .small()
                    .label("Remove group")
                    .on_click(
                        cx.listener(move |panel, _, _, cx| panel.remove_group(group_index, cx)),
                    ),
            );
            group_box = group_box.child(heading);
            for (condition_index, condition) in group.conditions.iter().enumerate() {
                group_box = group_box.child(self.render_condition(
                    Some(group_index),
                    condition_index,
                    condition,
                    &columns,
                    catalog.as_ref(),
                    cx,
                ));
            }
            group_box = group_box.child(
                Button::new(SharedString::from(format!(
                    "group-add-condition-{group_index}"
                )))
                .small()
                .label("Add condition")
                .disabled(total_conditions >= maximum_conditions)
                .on_click(cx.listener(move |panel, _, w, cx| {
                    panel.add_condition(Some(group_index), w, cx)
                })),
            );
            for (action_index, action) in group.actions.iter().enumerate() {
                group_box = group_box.child(self.render_action(
                    Some(group_index),
                    action_index,
                    action,
                    catalog.as_ref(),
                    editor.trigger.kind,
                    maximum_actions,
                    cx,
                ));
            }
            let weak_action = weak.clone();
            group_box = group_box.child(picker(
                SharedString::from(format!("group-add-action-{group_index}")),
                "Add action".to_owned(),
                action_choices.clone(),
                Rc::new(move |value, window, cx| {
                    if let Some(kind) = action_kind_from(&value) {
                        let _ = weak_action.update(cx, |panel, cx| {
                            panel.add_action(Some(group_index), kind, window, cx)
                        });
                    }
                }),
            ));
            body = body.child(group_box);
        }
        body = body.child(
            Button::new("add-automation-group")
                .small()
                .label("Add conditional actions")
                .disabled(editor.groups.len() >= MAX_GROUPS || total_actions >= maximum_actions)
                .on_click(cx.listener(|panel, _, w, cx| panel.add_group(w, cx))),
        );
        let mut footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .p_3();
        footer = footer
            .child(
                Checkbox::new("automation-enabled")
                    .label("Enable automation")
                    .checked(editor.enabled)
                    .disabled(editor.saving)
                    .on_click(cx.listener(|panel, checked, _, cx| {
                        if let Some(editor) = &mut panel.editor {
                            editor.enabled = *checked;
                            cx.notify();
                        }
                    })),
            )
            .child(
                Button::new("save-automation")
                    .small()
                    .label(if editor.saving {
                        "Saving…"
                    } else if editor.is_existing() {
                        "Update automation"
                    } else {
                        "Create automation"
                    })
                    .disabled(
                        editor.saving || !editor.trigger_chosen || editor.total_actions() == 0,
                    )
                    .on_click(cx.listener(|panel, _, w, cx| panel.save_editor(w, cx))),
            );
        let _ = window;
        sheet = sheet.child(body).child(footer);
        layer = layer.child(sheet);
        if let Some(composer) = &editor.composer {
            layer = layer.child(self.render_composer(composer, editor.trigger.kind, cx));
        }
        layer.into_any_element()
    }
}

impl AutomationPanel {
    fn render_condition(
        &self,
        group: Option<usize>,
        index: usize,
        condition: &ConditionDraft,
        columns: &[AutomationChoice],
        catalog: Option<&AutomationCatalog>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let mut row = div().flex().items_center().flex_wrap().gap_2();
        let weak_column = weak.clone();
        row = row.child(picker(
            SharedString::from(format!("condition-column-{:?}-{index}", group)),
            choice_name(columns, condition.column_id.as_deref()),
            columns.to_vec(),
            Rc::new(move |value, window, cx| {
                let _ = weak_column.update(cx, |panel, cx| {
                    panel.set_condition_column(group, index, value, window, cx)
                });
            }),
        ));
        let weak_operator = weak.clone();
        row = row.child(picker(
            SharedString::from(format!("condition-operator-{:?}-{index}", group)),
            operator_label(condition.operator).to_owned(),
            vec![
                choice("is", "is"),
                choice("is_not", "is not"),
                choice("is_empty", "is empty"),
                choice("is_not_empty", "is not empty"),
            ],
            Rc::new(move |value, _, cx| {
                if let Some(operator) = operator_from(&value) {
                    let _ = weak_operator.update(cx, |panel, cx| {
                        panel.set_condition_operator(group, index, operator, cx)
                    });
                }
            }),
        ));
        if matches!(
            condition.operator,
            AutomationConditionOperator::Is | AutomationConditionOperator::IsNot
        ) {
            let kind = column_choice_kind(catalog, condition.column_id.as_deref());
            if is_text_kind(kind) {
                row = row.child(Input::new(&condition.input).small());
            } else {
                let choices = trigger_value_choices(catalog, condition.column_id.as_deref());
                let handle = weak.clone();
                row = row.child(picker(
                    SharedString::from(format!("condition-value-{:?}-{index}", group)),
                    choice_name(&choices, condition.value.as_deref()),
                    choices,
                    Rc::new(move |value, _, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_condition_choice(group, index, text_value(&value), cx)
                        });
                    }),
                ));
            }
        }
        row = row.child(
            Button::new(SharedString::from(format!(
                "condition-remove-{:?}-{index}",
                group
            )))
            .small()
            .label("Remove")
            .on_click(cx.listener(move |panel, _, _, cx| {
                panel.remove_condition(group, index, cx)
            })),
        );
        row.into_any_element()
    }
}

impl AutomationPanel {
    fn render_action(
        &self,
        group: Option<usize>,
        index: usize,
        action: &ActionDraft,
        catalog: Option<&AutomationCatalog>,
        trigger: AutomationTriggerKind,
        maximum_actions: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let mut card = div()
            .id(SharedString::from(format!("action-{:?}-{index}", group)))
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .p_2();
        let mut line = div().flex().items_center().flex_wrap().gap_2();
        let kinds = catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .actions
                    .iter()
                    .map(|kind| choice(wire(kind), action_label(*kind)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let weak_kind = weak.clone();
        line = line
            .child(Label::new(if index == 0 { "Then" } else { "and then" }))
            .child(picker(
                SharedString::from(format!("action-kind-{:?}-{index}", group)),
                action_label(action.kind).to_owned(),
                kinds,
                Rc::new(move |value, window, cx| {
                    if let Some(kind) = action_kind_from(&value) {
                        let _ = weak_kind.update(cx, |panel, cx| {
                            panel.set_action_kind(group, index, kind, window, cx)
                        });
                    }
                }),
            ));
        match action.kind {
            AutomationActionKind::AssignPeople | AutomationActionKind::ReplacePeople => {
                line = line.child(Label::new(
                    if action.kind == AutomationActionKind::AssignPeople {
                        "add"
                    } else {
                        "replace with"
                    },
                ));
                line = line.child(self.render_people(action, group, index, catalog, cx));
                line = line.child(Label::new(
                    if action.kind == AutomationActionKind::AssignPeople {
                        "to"
                    } else {
                        "in"
                    },
                ));
                let choices = named_columns(catalog, "people");
                let handle = weak.clone();
                line = line.child(picker(
                    SharedString::from(format!("action-column-{:?}-{index}", group)),
                    choice_name(&choices, action.column_id.as_deref()),
                    choices,
                    Rc::new(move |value, window, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_column(group, index, value, window, cx)
                        });
                    }),
                ));
            }
            AutomationActionKind::ClearPeople => {
                let choices = named_columns(catalog, "people");
                let handle = weak.clone();
                line = line.child(Label::new("from")).child(picker(
                    SharedString::from(format!("action-column-{:?}-{index}", group)),
                    choice_name(&choices, action.column_id.as_deref()),
                    choices,
                    Rc::new(move |value, window, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_column(group, index, value, window, cx)
                        });
                    }),
                ));
            }
            AutomationActionKind::AssignCreator => {
                let choices = named_columns(catalog, "people");
                let handle = weak.clone();
                line = line.child(Label::new("as")).child(picker(
                    SharedString::from(format!("action-column-{:?}-{index}", group)),
                    choice_name(&choices, action.column_id.as_deref()),
                    choices,
                    Rc::new(move |value, window, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_column(group, index, value, window, cx)
                        });
                    }),
                ));
            }
            AutomationActionKind::SetDate | AutomationActionKind::PushDate => {
                let choices = named_columns(catalog, "date");
                let handle = weak.clone();
                line = line.child(picker(
                    SharedString::from(format!("action-column-{:?}-{index}", group)),
                    choice_name(&choices, action.column_id.as_deref()),
                    choices,
                    Rc::new(move |value, window, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_column(group, index, value, window, cx)
                        });
                    }),
                ));
                if action.kind == AutomationActionKind::SetDate {
                    let handle = weak.clone();
                    line = line.child(Label::new("to")).child(picker(
                        SharedString::from(format!("action-basis-{:?}-{index}", group)),
                        match action.date_basis {
                            AutomationDateBasis::Today => "Today".to_owned(),
                            AutomationDateBasis::ItemCreation => "item creation date".to_owned(),
                        },
                        vec![
                            choice("today", "Today"),
                            choice("item_creation", "item creation date"),
                        ],
                        Rc::new(move |value, _, cx| {
                            if let Some(basis) = date_basis_from(&value) {
                                let _ = handle.update(cx, |panel, cx| {
                                    panel.set_action_date_basis(group, index, basis, cx)
                                });
                            }
                        }),
                    ));
                }
                line = line
                    .child(Label::new(if action.kind == AutomationActionKind::SetDate {
                        "plus"
                    } else {
                        "by"
                    }))
                    .child(Input::new(&action.days_input).small().w(gpui_kit::px(72.)))
                    .child(Label::new("days"));
            }
            AutomationActionKind::MoveToGroup => {
                let choices = catalog
                    .and_then(|catalog| catalog.columns.iter().find(|column| column.id == "Group"))
                    .map(|column| column.choices.clone())
                    .unwrap_or_default();
                let handle = weak.clone();
                line = line.child(picker(
                    SharedString::from(format!("action-group-{:?}-{index}", group)),
                    choice_name(&choices, action.value.as_deref()),
                    choices,
                    Rc::new(move |value, _, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_choice(group, index, text_value(&value), cx)
                        });
                    }),
                ));
            }
            AutomationActionKind::ChangeStatus => {
                let columns = named_columns(catalog, "status");
                let handle = weak.clone();
                line = line.child(picker(
                    SharedString::from(format!("action-column-{:?}-{index}", group)),
                    choice_name(&columns, action.column_id.as_deref()),
                    columns,
                    Rc::new(move |value, window, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_column(group, index, value, window, cx)
                        });
                    }),
                ));
                let choices = trigger_value_choices(catalog, action.column_id.as_deref());
                let handle = weak.clone();
                line = line.child(Label::new("to")).child(picker(
                    SharedString::from(format!("action-status-{:?}-{index}", group)),
                    choice_name(&choices, action.value.as_deref()),
                    choices,
                    Rc::new(move |value, _, cx| {
                        let _ = handle.update(cx, |panel, cx| {
                            panel.set_action_choice(group, index, text_value(&value), cx)
                        });
                    }),
                ));
            }
            AutomationActionKind::Notify => {
                line = line
                    .child(
                        Button::new(SharedString::from(format!(
                            "action-message-{:?}-{index}",
                            group
                        )))
                        .small()
                        .label(if action.message.is_some() {
                            "edit message"
                        } else {
                            "write a notification"
                        })
                        .on_click(cx.listener(move |panel, _, w, cx| {
                            panel.open_composer(group, index, w, cx)
                        })),
                    )
                    .child(Label::new("to"));
                let recipients = recipient_choices(trigger);
                let handle = weak.clone();
                line = line.child(picker(
                    SharedString::from(format!("action-recipient-{:?}-{index}", group)),
                    recipient_label(action.recipient, Some(trigger)).to_owned(),
                    recipients,
                    Rc::new(move |value, _, cx| {
                        if let Some(recipient) = recipient_from(&value) {
                            let _ = handle.update(cx, |panel, cx| {
                                panel.set_action_recipient(group, index, recipient, cx)
                            });
                        }
                    }),
                ));
                if action.recipient == AutomationRecipient::People {
                    line = line.child(self.render_people(action, group, index, catalog, cx));
                }
            }
        }
        card = card.child(line);
        let mut controls = div().flex().items_center().gap_2();
        if index > 0 {
            controls = controls.child(
                Button::new(SharedString::from(format!("action-up-{:?}-{index}", group)))
                    .small()
                    .label("Move up")
                    .on_click(
                        cx.listener(move |panel, _, _, cx| panel.move_action_up(group, index, cx)),
                    ),
            );
        }
        controls = controls.child(
            Button::new(SharedString::from(format!(
                "action-remove-{:?}-{index}",
                group
            )))
            .small()
            .label("Remove")
            .on_click(cx.listener(move |panel, _, _, cx| panel.remove_action(group, index, cx))),
        );
        let _ = maximum_actions;
        card.child(controls).into_any_element()
    }
}

impl AutomationPanel {
    fn render_people(
        &self,
        action: &ActionDraft,
        group: Option<usize>,
        index: usize,
        catalog: Option<&AutomationCatalog>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut people = div().flex().items_center().flex_wrap().gap_1();
        for id in &action.person_ids {
            let id = id.clone();
            people = people.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded_full()
                    .bg(cx.theme().muted)
                    .px_2()
                    .child(Label::new(person_name(catalog, &id)).text_xs())
                    .child(
                        Button::new(SharedString::from(format!(
                            "remove-person-{:?}-{index}-{id}",
                            group
                        )))
                        .small()
                        .label("x")
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |panel, _, _, cx| {
                                let remaining = panel
                                    .editor
                                    .as_ref()
                                    .and_then(|editor| editor.action(group, index))
                                    .map(|action| {
                                        action
                                            .person_ids
                                            .iter()
                                            .filter(|person| **person != id)
                                            .cloned()
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                panel.set_action_people(group, index, remaining, cx);
                            }
                        })),
                    ),
            );
        }
        let choices = catalog
            .map(|catalog| {
                catalog
                    .people
                    .iter()
                    .filter(|person| person.can_mention && !action.person_ids.contains(&person.id))
                    .map(|person| choice(person.id.clone(), person.name.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let handle = cx.weak_entity();
        people = people.child(picker(
            SharedString::from(format!("action-people-{:?}-{index}", group)),
            if action.person_ids.is_empty() {
                "someone".to_owned()
            } else {
                "+ person".to_owned()
            },
            choices,
            Rc::new(move |id, _, cx| {
                let _ = handle.update(cx, |panel, cx| {
                    let mut people = panel
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.action(group, index))
                        .map(|action| action.person_ids.clone())
                        .unwrap_or_default();
                    people.push(id);
                    panel.set_action_people(group, index, people, cx);
                });
            }),
        ));
        people.into_any_element()
    }

    fn render_composer(
        &self,
        composer: &Composer,
        trigger: AutomationTriggerKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let overlay = div()
            .id("automation-composer-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.92));
        let mut dialog = div()
            .id("automation-composer")
            .w(gpui_kit::px(560.))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_4()
            .child(Label::new("Create a notification").text_lg())
            .child(Label::new("Subject line"))
            .child(Input::new(&composer.title).id("notification-title"))
            .child(Label::new("Message"))
            .child(Textarea::new(&composer.message));
        let mut tokens = div().flex().items_center().flex_wrap().gap_1();
        for (label, token) in notification_tokens(trigger) {
            tokens = tokens.child(
                Button::new(SharedString::from(format!("notification-token-{token}")))
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |panel, _, w, cx| panel.append_token(token, w, cx))),
            );
        }
        dialog = dialog.child(tokens).child(
            div().flex().items_center().justify_end().child(
                Button::new("notification-done")
                    .small()
                    .label("Done")
                    .on_click(cx.listener(|panel, _, _, cx| panel.finish_composer(cx))),
            ),
        );
        overlay.child(dialog).into_any_element()
    }
}

fn trigger_value_choices(
    catalog: Option<&AutomationCatalog>,
    column: Option<&str>,
) -> Vec<AutomationChoice> {
    let mut choices = vec![choice("", "Anything")];
    if let Some(column) = column.and_then(|id| {
        catalog?
            .columns
            .iter()
            .find(|column| column.id == id)
    }) {
        if column.kind == "status" {
            choices.push(choice("", "Empty"));
        }
        choices.extend(column.choices.iter().cloned());
    }
    choices
}

fn choice_name(choices: &[AutomationChoice], value: Option<&str>) -> String {
    value
        .and_then(|value| choices.iter().find(|choice| choice.value == value))
        .map(|choice| choice.name.clone())
        .unwrap_or_else(|| match value {
            Some(value) if !value.is_empty() => value.to_owned(),
            _ => "Anything".to_owned(),
        })
}

fn recipient_choices(trigger: AutomationTriggerKind) -> Vec<AutomationChoice> {
    let mut choices = vec![
        choice("people", "people"),
        choice("assigned", "Assigned"),
        choice("creator", "item creator"),
    ];
    if matches!(
        trigger,
        AutomationTriggerKind::PersonAssigned
            | AutomationTriggerKind::PersonMentioned
            | AutomationTriggerKind::UpdatePosted
            | AutomationTriggerKind::ReplyPosted
            | AutomationTriggerKind::ReactionAdded
    ) {
        choices.push(choice(
            "trigger_people",
            recipient_label(AutomationRecipient::TriggerPeople, Some(trigger)),
        ));
    }
    choices
}

fn notification_tokens(trigger: AutomationTriggerKind) -> Vec<(&'static str, &'static str)> {
    let mut tokens = vec![
        ("Item name", "{item.name}"),
        ("Board name", "{board.name}"),
        ("Item ID", "{item.id}"),
    ];
    if matches!(
        trigger,
        AutomationTriggerKind::PersonMentioned
            | AutomationTriggerKind::UpdatePosted
            | AutomationTriggerKind::ReplyPosted
            | AutomationTriggerKind::ReactionAdded
    ) {
        tokens.extend([
            ("Update link", "{post.url}"),
            ("Update ID", "{post.id}"),
            ("Person who triggered", "{actor.name}"),
        ]);
    }
    if trigger == AutomationTriggerKind::ReactionAdded {
        tokens.push(("Reaction", "{reaction.type}"));
    }
    tokens
}


