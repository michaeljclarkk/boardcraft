use crate::{
    grouped_entry,
    native_table::{B, NativeTable},
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit};
use uuid::Uuid;

pub fn current_access() {
    let f = NativeTable::new();
    let before = f.state();
    let client = f.client();
    let id = Uuid::new_v4();
    let edit = NameEdit {
        title: " Coral ".into(),
        revision: 1,
    };
    let create = |client: &EditClient, board, group: &str, title: &str| {
        client.create_main_table_item(board, group, id, title)
    };
    let rename =
        |client: &EditClient, board, item| client.rename_main_table_item(board, item, &edit);
    let anonymous = EditClient::new(f.selection.host.port);
    assert_eq!(
        create(&anonymous, SELECTED, B, "Ink"),
        Err(EditError::SessionRejected)
    );
    assert_eq!(
        rename(&anonymous, SELECTED, Uuid::from_u128(61)),
        Err(EditError::SessionRejected)
    );
    for board in [HIDDEN, f.f.board] {
        assert_eq!(
            create(&client, board, B, "Ink"),
            Err(EditError::PermissionDenied)
        );
        assert_eq!(
            rename(&client, board, Uuid::from_u128(61)),
            Err(EditError::PermissionDenied)
        );
    }
    for group in ["missing", "g-hidden", "g-peer"] {
        assert_eq!(
            create(&client, SELECTED, group, "Ink"),
            Err(EditError::InvalidDraft)
        );
    }
    for title in ["   ".to_owned(), "😀".repeat(128)] {
        assert_eq!(
            create(&client, SELECTED, B, &title),
            Err(EditError::InvalidDraft)
        );
        assert_eq!(
            client.rename_main_table_item(
                SELECTED,
                Uuid::from_u128(61),
                &NameEdit { title, revision: 1 }
            ),
            Err(EditError::InvalidDraft)
        );
    }
    assert_eq!(
        client.create_main_table_item(SELECTED, B, Uuid::nil(), "Ink"),
        Err(EditError::InvalidDraft)
    );
    for item in [62, 57, 67, 999] {
        assert_eq!(
            rename(&client, SELECTED, Uuid::from_u128(item)),
            Err(EditError::Missing)
        );
    }
    f.role("viewer");
    assert!(!client.read_main_table(SELECTED).unwrap().is_editable());
    assert_eq!(
        create(&client, SELECTED, B, "Ink"),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        rename(&client, SELECTED, Uuid::from_u128(61)),
        Err(EditError::PermissionDenied)
    );
    f.role("member");
    f.selection.membership(&f.f, false);
    assert_eq!(
        create(&client, SELECTED, B, "Ink"),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        rename(&client, SELECTED, Uuid::from_u128(61)),
        Err(EditError::PermissionDenied)
    );
    f.selection.membership(&f.f, true);
    grouped_entry::check(&f, &before, None);
    grouped_entry::check_name(&f, &before, "Ink", 1);

    let direct = EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap();
    assert_eq!(create(&direct, SELECTED, B, "Ink"), Err(EditError::Missing));
    assert_eq!(
        rename(&direct, SELECTED, Uuid::from_u128(61)),
        Err(EditError::Missing)
    );
    assert_eq!(
        client.rename(SELECTED, Uuid::from_u128(61), &edit),
        Err(EditError::Missing)
    );
    assert_eq!(client.read(SIBLING), Err(EditError::Missing));
    assert_eq!(direct.read(f.f.board).unwrap().len(), 2);
    assert_eq!(
        direct
            .rename(
                f.f.board,
                Uuid::from_u128(42),
                &NameEdit {
                    title: "Review".into(),
                    revision: 1
                }
            )
            .unwrap()
            .title,
        "Review"
    );

    // Commit refusal rolls back both the root and its creation provenance.
    f.f.connect().batch_execute("CREATE FUNCTION refuse_entry() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_entry AFTER INSERT ON board_items DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_entry()").unwrap();
    assert_eq!(
        create(&client, SELECTED, B, "Ink"),
        Err(EditError::HostFault)
    );
    grouped_entry::check(&f, &before, None);
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_entry ON board_items")
        .unwrap();
    let saved = create(&client, SELECTED, B, " Ink ").unwrap();
    assert_eq!(
        (saved.id, saved.title.as_str(), saved.revision),
        (id, "Ink", 1)
    );
    grouped_entry::check(&f, &before, Some((id, "Ink", 1)));
    // Explicit store-level creator replay remains safe, but the UI never uses it for recovery.
    assert_eq!(create(&client, SELECTED, B, " Ink ").unwrap(), saved);
    grouped_entry::check(&f, &before, Some((id, "Ink", 1)));
    assert_eq!(
        client.create_main_table_item(SELECTED, B, Uuid::from_u128(61), "Ink"),
        Err(EditError::Conflict)
    );
    f.role("owner");
    assert_eq!(create(&client, SELECTED, B, " Ink ").unwrap(), saved);
    let saved = rename(&client, SELECTED, id).unwrap();
    assert_eq!(
        (saved.id, saved.title.as_str(), saved.revision),
        (id, "Coral", 2)
    );
    grouped_entry::check(&f, &before, Some((id, "Coral", 2)));
    assert_eq!(
        client.rename_main_table_item(
            SELECTED,
            id,
            &NameEdit {
                title: "Changed".into(),
                revision: 99
            }
        ),
        Err(EditError::Conflict)
    );
    f.selection.session.revoke(&f.f);
    assert_eq!(
        create(&client, SELECTED, B, "Ink"),
        Err(EditError::SessionRejected)
    );
    assert_eq!(
        rename(&client, SELECTED, id),
        Err(EditError::SessionRejected)
    );
    // The final independent connection cannot use the now-revoked HTTP client.
    let state = f.state();
    let root = state["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == serde_json::json!(id))
        .unwrap();
    assert_eq!(root["title"], "Coral");
    assert_eq!(root["revision"], 2);
}
