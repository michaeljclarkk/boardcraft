use crate::{group_edit, native_selection::NativeSelection, selection::SELECTED};

pub fn fixture() -> NativeSelection {
    let f = NativeSelection::with_seed(|f, _| {
        group_edit::seed(f);
        f.connect()
            .execute("INSERT INTO main_tables VALUES($1,1)", &[&SELECTED])
            .unwrap();
        f.connect()
            .execute(
                "INSERT INTO main_table_folds VALUES($1,'g-old')",
                &[&SELECTED],
            )
            .unwrap();
    });
    f.role("owner");
    f
}
