use crate::{lifecycle, native_selection::NativeSelection, selection::SELECTED};

pub fn fixture() -> NativeSelection {
    let f = NativeSelection::with_seed(|f, actor| {
        lifecycle::seed(f, actor);
        f.connect().batch_execute("UPDATE board_items SET created_at='2000-01-01T00:00:00Z' WHERE id='00000000-0000-0000-0000-000000000039'").unwrap();
        f.connect()
            .execute(
                "UPDATE board_groups SET position=1 WHERE board_id=$1 AND id='g-20'",
                &[&SELECTED],
            )
            .unwrap();
    });
    f.role("owner");
    f
}
