use crate::{native_group_fields, native_selection::NativeSelection, selection::SELECTED};

pub fn fixture() -> NativeSelection {
    let f = native_group_fields::fixture();
    // Same compared records; this journey moves the folded second Work upward.
    f.f.connect()
        .execute(
            "UPDATE main_table_folds SET group_id='g-z' WHERE board_id=$1",
            &[&SELECTED],
        )
        .unwrap();
    f
}
