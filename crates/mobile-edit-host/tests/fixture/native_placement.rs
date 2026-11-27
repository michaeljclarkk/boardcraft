//! Packet 33/34 records, with no alternate placement rules or duplicate fixtures.
use crate::{move_state::MoveState, native_selection::NativeSelection};

pub fn fixture(reorder: bool) -> NativeSelection {
    if reorder {
        NativeSelection::new()
    } else {
        NativeSelection::with_seed(|f, actor| {
            MoveState::seed(f, actor);
        })
    }
}

pub const MOVE_FAULT: &str = "CREATE FUNCTION refuse_placement() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_placement AFTER UPDATE ON board_items DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_placement()";
pub const ORDER_FAULT: &str = "CREATE FUNCTION refuse_placement() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_placement AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_placement()";
