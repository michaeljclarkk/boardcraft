//! Automation rules, catalog, execution and history.
mod engine;
mod model;
mod rules;
mod store;

pub use engine::{Error as EngineError, Event, apply};
pub use model::*;
pub use store::{
    HISTORY_PAGE, RULE_PAGE, StoreError, catalog, history, list, parse_rule_id, rule_id, save,
    set_archived, set_enabled,
};
