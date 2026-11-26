use crate::{
    native_selection::NativeSelection,
    selection::{HIDDEN, SELECTED},
};
use boardcraft_mobile_edit_client::{EditClient, EditError};
use uuid::Uuid;

pub fn access_and_order() {
    let f = NativeSelection::new();
    let client = f.client();
    let before = f.state();
    let items = [67, 59, 61].map(|n| (Uuid::from_u128(n), 1));
    let op = Uuid::new_v4();
    let submit = |client: &EditClient, board, group: &str, revision, rows: &[(Uuid, i64)]| {
        client.move_main_table_selection(board, op, group, revision, rows)
    };
    let check = || {
        assert_eq!(f.state(), before);
        assert!(f.selection.session.present(&f.f));
    };
    f.check_access("member", true, true);
    assert_eq!(
        client.move_main_table_selection(SELECTED, Uuid::nil(), "g-90", 1, &items),
        Err(EditError::InvalidDraft)
    );
    check();
    assert_eq!(
        submit(
            &EditClient::new(f.selection.host.port),
            SELECTED,
            "g-90",
            1,
            &items
        ),
        Err(EditError::SessionRejected)
    );
    check();
    assert_eq!(
        submit(&client, HIDDEN, "g-90", 1, &items),
        Err(EditError::PermissionDenied)
    );
    check();
    let direct = EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap();
    assert_eq!(
        submit(&direct, SELECTED, "g-90", 1, &items),
        Err(EditError::Missing)
    );
    check();
    for group in ["missing", "g-30", "g-70"] {
        assert_eq!(
            submit(&client, SELECTED, group, 1, &items),
            Err(EditError::InvalidDraft)
        );
        check();
    }
    for rows in [
        vec![],
        vec![items[0], items[0]],
        vec![items[0], (Uuid::from_u128(57), 1)],
        vec![items[0], (Uuid::from_u128(62), 1)],
        vec![items[0], (Uuid::from_u128(999), 1)],
        vec![items[0], (items[1].0, 2)],
    ] {
        let expected = if rows
            .iter()
            .any(|(id, _)| [57, 62, 999].contains(&id.as_u128()))
        {
            EditError::Missing
        } else if rows.last().is_some_and(|(_, r)| *r == 2) {
            EditError::Conflict
        } else {
            EditError::InvalidDraft
        };
        assert_eq!(submit(&client, SELECTED, "g-90", 1, &rows), Err(expected));
        check();
    }
    assert_eq!(
        submit(&client, SELECTED, "g-90", 2, &items),
        Err(EditError::Conflict)
    );
    check();
    f.role("viewer");
    f.check_access("viewer", true, true);
    assert!(!client.read_main_table(SELECTED).unwrap().is_editable());
    assert_eq!(
        submit(&client, SELECTED, "g-90", 1, &items),
        Err(EditError::PermissionDenied)
    );
    check();
    f.role("member");
    f.selection.membership(&f.f, false);
    f.check_access("member", false, true);
    assert_eq!(
        submit(&client, SELECTED, "g-90", 1, &items),
        Err(EditError::PermissionDenied)
    );
    check();
    f.selection.membership(&f.f, true);
    f.check_access("member", true, true);
    f.f.connect().batch_execute("CREATE FUNCTION refuse_batch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_batch AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_batch()").unwrap();
    assert_eq!(
        submit(&client, SELECTED, "g-90", 1, &items),
        Err(EditError::HostFault)
    );
    check();
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_batch ON group_reorder_receipts")
        .unwrap();
    submit(&client, SELECTED, "g-90", 1, &items).unwrap();
    let saved = before.batch(&f.f, f.member, "g-90", &[73, 77, 67, 59, 61], &[67, 59, 61]);
    // Reconnect: display order comes from GET, not the affected-ID receipt.
    let read = f.client().read_main_table(SELECTED).unwrap();
    assert_eq!(read.revision(), 2);
    assert_eq!(
        read.groups()[0]
            .items()
            .iter()
            .map(|r| (r.id.as_u128(), r.revision))
            .collect::<Vec<_>>(),
        [(73, 1), (77, 1), (67, 2), (59, 2), (61, 2)]
    );
    submit(&f.client(), SELECTED, "g-90", 1, &items).unwrap();
    assert_eq!(f.state(), saved);
    assert_eq!(
        submit(
            &client,
            SELECTED,
            "g-90",
            1,
            &[items[2], items[1], items[0]]
        ),
        Err(EditError::Conflict)
    );
    assert_eq!(f.state(), saved);
    f.role("viewer");
    assert_eq!(
        submit(&client, SELECTED, "g-90", 1, &items),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(f.state(), saved);
    f.role("member");
    f.selection.session.revoke(&f.f);
    f.check_access("member", true, false);
    assert_eq!(
        submit(&client, SELECTED, "g-90", 1, &items),
        Err(EditError::SessionRejected)
    );
    assert_eq!(f.state(), saved);
}
