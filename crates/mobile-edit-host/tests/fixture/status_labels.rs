use super::{
    browser_transport::send,
    fixture::Fixture,
    selection::{self, SELECTED, SIBLING},
};
use boardcraft_account::Role;
use boardcraft_board::{BoardId, store};
use boardcraft_mobile_edit_client::{
    EditClient, EditError, StatusChoice, StatusLabelEdit, StatusLabels,
};
use boardcraft_mobile_edit_host::Host;
use serde_json::json;
use uuid::Uuid;

#[test]
fn status_labels_scoped_patch_reopen_identity_and_permissions() {
    let f = Fixture::start();
    let s = selection::Selection::new(&f);
    let c = EditClient::with_bearer(s.host.port, s.token.encoded()).unwrap();
    let table = c.read_status(SELECTED).unwrap();
    let col = table.column_id;
    let patch = StatusLabelEdit {
        label: Some("  Reviewed  ".into()),
        color: None,
    };
    assert!(!c.read_status_labels(SELECTED, col).unwrap().can_edit);
    assert_eq!(
        c.save_status_label(SELECTED, col, "done", &patch),
        Err(EditError::PermissionDenied)
    );
    f.connect()
        .execute(
            "UPDATE accounts SET role='owner' WHERE username='reader'",
            &[],
        )
        .unwrap();
    assert!(c.read_status_labels(SELECTED, col).unwrap().can_edit);
    let saved = c
        .set_status(SELECTED, col, Uuid::from_u128(61), 0, Some("done".into()))
        .unwrap();
    assert_eq!(
        c.save_status_label(SELECTED, col, "done", &patch)
            .unwrap()
            .label,
        "  Reviewed  "
    );
    let color = StatusLabelEdit {
        label: None,
        color: Some("#123aBc".into()),
    };
    let recolored = c.save_status_label(SELECTED, col, "done", &color).unwrap();
    assert_eq!(
        (&*recolored.label, &*recolored.color),
        ("  Reviewed  ", "#123aBc")
    );
    assert_eq!(
        c.save_status_label(SELECTED, col, "done", &color).unwrap(),
        recolored
    );
    assert_eq!(
        c.read_status(SELECTED)
            .unwrap()
            .cells
            .iter()
            .find(|v| v.item_id == saved.item_id),
        Some(&saved)
    );
    assert_eq!(
        c.read_status(SIBLING)
            .unwrap()
            .choices
            .iter()
            .find(|v| v.key == "done")
            .unwrap()
            .label,
        "Done"
    );
    assert_eq!(
        c.save_status_label(SIBLING, col, "done", &patch),
        Err(EditError::Missing)
    );
    let default = StatusLabelEdit {
        label: Some("Unassigned".into()),
        color: Some("#ffffff".into()),
    };
    assert_eq!(
        c.save_status_label(SELECTED, col, "empty", &default)
            .unwrap()
            .value(),
        None
    );
    assert_eq!(
        c.set_status(
            SELECTED,
            col,
            Uuid::from_u128(61),
            saved.revision,
            Some("empty".into())
        ),
        Err(EditError::InvalidDraft)
    );
    let key = format!("l-{}", Uuid::new_v4());
    assert_eq!(
        c.save_status_label(SELECTED, col, &key, &patch),
        Err(EditError::InvalidDraft)
    );
    let new = StatusLabelEdit {
        label: Some("New label".into()),
        color: Some("#ff158a".into()),
    };
    let added = c.save_status_label(SELECTED, col, &key, &new).unwrap();
    assert_eq!(
        c.save_status_label(SELECTED, col, &key, &new).unwrap(),
        added
    );
    let t = c.read_status(SELECTED).unwrap();
    assert_eq!(
        t.choices.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["working", "done", "stuck", "empty", &key]
    );
    assert_eq!(
        c.set_status(
            SELECTED,
            col,
            saved.item_id,
            saved.revision,
            Some(key.clone())
        )
        .unwrap()
        .revision,
        2
    );
    for invalid in [
        StatusLabelEdit::default(),
        StatusLabelEdit {
            label: Some("😀".repeat(81)),
            color: None,
        },
        StatusLabelEdit {
            label: None,
            color: Some("red".into()),
        },
    ] {
        assert_eq!(
            c.save_status_label(SELECTED, col, "done", &invalid),
            Err(EditError::InvalidDraft)
        );
    }
    assert!(
        c.save_status_label(
            SELECTED,
            col,
            "done",
            &StatusLabelEdit {
                label: Some("".into()),
                color: None
            }
        )
        .is_ok()
    );
    let reopened =
        Host::start_with_navigation(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
    let fresh = EditClient::with_bearer(reopened.port, s.token.encoded()).unwrap();
    assert_eq!(
        fresh
            .read_status_labels(SELECTED, col)
            .unwrap()
            .choices
            .last(),
        Some(&added)
    );
    f.connect().execute("INSERT INTO status_labels SELECT $1,'l-'||gen_random_uuid(),'More','#123456',n FROM generate_series(5,249) n",&[&col]).unwrap();
    assert_eq!(
        c.save_status_label(SELECTED, col, &format!("l-{}", Uuid::new_v4()), &new),
        Err(EditError::InvalidDraft)
    );
    s.membership(&f, false);
    assert_eq!(
        fresh.read_status_labels(SELECTED, col),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        fresh.save_status_label(SELECTED, col, "done", &color),
        Err(EditError::PermissionDenied)
    );
}

#[test]
fn status_labels_browser_binding_and_native_share_configuration() {
    let f = Fixture::start();
    let password = Uuid::new_v4().to_string();
    let owner = f.seed_account("labels-owner", &password, Role::Owner);
    let catalog = selection::Catalog::new(&f, owner);
    let host = Host::start_web_api(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
    let authority = format!("127.0.0.1:{}", host.port);
    let origin = format!("http://{authority}");
    let login = send(
        host.port,
        "POST",
        "/web/api/session",
        &authority,
        &[("Origin", &origin), ("Boardcraft-Web", "login")],
        &json!({"identifier":"labels-owner","password":password}).to_string(),
    );
    assert_eq!(login.status, 200);
    let binding: String = serde_json::from_str(&login.body).unwrap();
    let cookie = login.cookie.as_deref().unwrap().split(';').next().unwrap();
    let pair = [("Cookie", cookie), ("Boardcraft-Web", binding.as_str())];
    let mut db = f.connect();
    let col: Uuid = db
        .query_one(
            "SELECT id FROM status_columns WHERE board_id=$1",
            &[&SELECTED],
        )
        .unwrap()
        .get(0);
    let path = format!("/web/api/navigation/status/{SELECTED}/{col}/labels");
    let reply = send(host.port, "GET", &path, &authority, &pair, "");
    assert_eq!(reply.status, 200);
    let labels: StatusLabels = serde_json::from_str(&reply.body).unwrap();
    assert!(labels.can_edit && labels.choices.len() == 4);
    let url = format!("{path}/empty");
    let patch = json!({"label":"Awaiting review","color":"#225091"}).to_string();
    assert_eq!(
        send(host.port, "PUT", &url, &authority, &pair, &patch).status,
        403
    );
    let reply = send(
        host.port,
        "PUT",
        &url,
        &authority,
        &[pair[0], pair[1], ("Origin", &origin)],
        &patch,
    );
    assert_eq!(reply.status, 200);
    let saved: StatusChoice = serde_json::from_str(&reply.body).unwrap();
    let mut tx = db.transaction().unwrap();
    let table =
        boardcraft_board::columns::read_status(&mut tx, BoardId::new(SELECTED).unwrap()).unwrap();
    assert_eq!(
        table
            .choices
            .iter()
            .find(|c| c.key == "empty")
            .unwrap()
            .label,
        saved.label
    );
    assert!(
        table
            .cells
            .iter()
            .all(|c| c.value.is_none() && c.revision == 0)
    );
    tx.commit().unwrap();
    store::migrate(&mut db).unwrap();
    catalog.membership(&f, false);
    assert_eq!(
        send(host.port, "GET", &path, &authority, &pair, "").status,
        403
    );
}

#[test]
fn status_labels_migration_preserves_cell_values_and_revisions() {
    let f = Fixture::start();
    let mut db = f.connect();
    let col: Uuid = db
        .query_one(
            "SELECT id FROM status_columns WHERE board_id=$1",
            &[&f.board],
        )
        .unwrap()
        .get(0);
    db.execute(
        "INSERT INTO status_cells VALUES ($1,$2,$3,'done',7)",
        &[&f.board, &col, &super::fixture::FIRST],
    )
    .unwrap();
    db.batch_execute(
        "ALTER TABLE status_cells DROP CONSTRAINT status_cells_column_id_value_fkey;
        ALTER TABLE status_cells ADD FOREIGN KEY(value) REFERENCES status_choices(key);
        DROP TRIGGER status_column_labels ON status_columns; DROP FUNCTION seed_status_labels();
        DROP TABLE status_labels; ALTER TABLE status_columns DROP COLUMN revision;
        DELETE FROM rustday_migrations WHERE version=21",
    )
    .unwrap();
    store::migrate(&mut db).unwrap();
    store::migrate(&mut db).unwrap();
    let mut tx = db.transaction().unwrap();
    let table =
        boardcraft_board::columns::read_status(&mut tx, BoardId::new(f.board).unwrap()).unwrap();
    assert_eq!(table.choices.len(), 4);
    let cell = table
        .cells
        .iter()
        .find(|c| c.item == super::fixture::FIRST)
        .unwrap();
    assert_eq!((cell.value.as_deref(), cell.revision), (Some("done"), 7));
    let before: i64 = tx
        .query_one("SELECT revision FROM boards WHERE id=$1", &[&f.board])
        .unwrap()
        .get(0);
    let board = BoardId::new(f.board).unwrap();
    boardcraft_board::columns::save_status_label(
        &mut tx,
        board,
        col,
        "done",
        Some("Reviewed"),
        None,
    )
    .unwrap();
    boardcraft_board::columns::save_status_label(
        &mut tx,
        board,
        col,
        "done",
        Some("Reviewed"),
        None,
    )
    .unwrap();
    let after: i64 = tx
        .query_one("SELECT revision FROM boards WHERE id=$1", &[&f.board])
        .unwrap()
        .get(0);
    assert_eq!(after, before + 1);
    tx.commit().unwrap();
}
