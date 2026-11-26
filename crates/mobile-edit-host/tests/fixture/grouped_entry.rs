//! Independent reconnect oracle shared with the Windows entry journeys.
use crate::{
    native_table::{A, B, C, NativeTable},
    selection::SELECTED,
};
use boardcraft_mobile_edit_client::SavedItem;
use serde_json::{Value, json};
use uuid::Uuid;

pub fn check(f: &NativeTable, before: &Value, created: Option<(Uuid, &str, i64)>) {
    check_storage(f, before, created);
    let expected = expected(created);
    let read = f.client().read_main_table(SELECTED).unwrap();
    assert_eq!(
        read.groups()
            .iter()
            .map(|g| (g.id(), g.name(), g.items().len()))
            .collect::<Vec<_>>(),
        [
            (A, "Work", 1),
            (B, "Work", 2 + usize::from(created.is_some())),
            (C, "Reserve", 0)
        ]
    );
    assert_eq!(
        read.groups()
            .iter()
            .flat_map(|g| g.items())
            .cloned()
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(f.client().read_selected_board(SELECTED).unwrap(), expected);
}

pub fn check_storage(f: &NativeTable, before: &Value, created: Option<(Uuid, &str, i64)>) {
    let mut state = f.state();
    if let Some((id, title, revision)) = created {
        let items = state["items"].as_array_mut().unwrap();
        let ix = items.iter().position(|r| r["id"] == json!(id)).unwrap();
        let row = items.remove(ix);
        for (key, value) in [
            ("board_id", json!(SELECTED)),
            ("group_id", json!(B)),
            ("position", json!(9)),
            ("title", json!(title)),
            ("revision", json!(revision)),
            ("created_by", json!(f.member)),
            ("creation_group_id", json!(B)),
            ("creation_title", json!("Ink")),
            ("state", json!(0)),
        ] {
            assert_eq!(row[key], value, "new root {key}");
        }
        assert!(!row["created_at"].is_null());
    }
    assert_eq!(state, *before, "only the newly created root may change");
    f.f.check_board(SELECTED, &expected(created));
}

fn expected(created: Option<(Uuid, &str, i64)>) -> Vec<SavedItem> {
    let mut expected: Vec<_> = [59, 61, 63]
        .into_iter()
        .map(|n| SavedItem {
            id: Uuid::from_u128(n),
            title: "Ink".into(),
            revision: 1,
        })
        .collect();
    if let Some((id, title, revision)) = created {
        expected.push(SavedItem {
            id,
            title: title.into(),
            revision,
        });
    }
    expected
}

pub fn check_name(f: &NativeTable, before: &Value, title: &str, revision: i64) {
    let mut expected_state = before.clone();
    let row = expected_state["items"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["id"] == json!(Uuid::from_u128(61)))
        .unwrap();
    row["title"] = json!(title);
    row["revision"] = json!(revision);
    assert_eq!(f.state(), expected_state);
    let mut rows = expected(None);
    rows[1].title = title.into();
    rows[1].revision = revision;
    f.f.check_board(SELECTED, &rows);
}
