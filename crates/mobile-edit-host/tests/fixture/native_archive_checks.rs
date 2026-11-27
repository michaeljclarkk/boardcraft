use crate::{
    native_archive,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_mobile_edit_client::{EditClient, EditError};
use uuid::Uuid;

pub fn access_and_scope() {
    let f = native_archive::fixture();
    let client = f.client();
    let initial = f.state();
    let root = |c: &EditClient, board, id, op, b, i, restore| {
        c.archive_main_table_item(board, Uuid::from_u128(id), op, b, i, restore)
    };
    let group =
        |c: &EditClient, board, id, b, restore| c.archive_main_table_group(board, id, b, restore);
    let op = Uuid::new_v4();
    let unchanged = || assert_eq!(f.state(), initial);
    let anonymous = EditClient::new(f.selection.host.port);
    let direct = EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap();
    for (c, board, error) in [
        (&anonymous, SELECTED, EditError::SessionRejected),
        (&client, HIDDEN, EditError::PermissionDenied),
        (&direct, SELECTED, EditError::Missing),
    ] {
        assert!(matches!(c.read_main_table_archive(board, None), Err(e) if e == error));
        assert!(matches!(root(c, board, 67, op, 1, 1, false), Err(e) if e == error));
        assert_eq!(group(c, board, "g-90", 1, false), Err(error));
        unchanged();
    }
    f.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$1 WHERE board_id=$2",
            &[&f.member, &SELECTED],
        )
        .unwrap();
    for role in ["member", "viewer"] {
        f.role(role);
        f.check_access(role, true, true);
        let table = client.read_main_table(SELECTED).unwrap();
        assert_eq!(table.is_editable(), role == "member");
        assert!(!table.is_manageable());
        let page = client.read_main_table_archive(SELECTED, None).unwrap();
        assert!(!page.is_manageable());
        assert_eq!(page.total(), 1);
        assert!(!page.items()[0].is_restorable());
        assert_eq!(
            root(&client, SELECTED, 67, op, 1, 1, false),
            Err(EditError::PermissionDenied)
        );
        assert_eq!(
            group(&client, SELECTED, "g-90", 1, false),
            Err(EditError::PermissionDenied)
        );
        assert_eq!(
            group(&client, SELECTED, "g-30", 1, true),
            Err(EditError::PermissionDenied)
        );
        unchanged();
    }
    f.role("owner");
    f.selection.membership(&f.f, false);
    assert!(matches!(
        client.read_main_table_archive(SELECTED, None),
        Err(EditError::PermissionDenied)
    ));
    assert_eq!(
        root(&client, SELECTED, 67, op, 1, 1, false),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        group(&client, SELECTED, "g-90", 1, false),
        Err(EditError::PermissionDenied)
    );
    unchanged();
    f.selection.membership(&f.f, true);
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        f.f.connect().execute(sql, &[&SELECTED, &false]).unwrap();
        assert!(matches!(
            client.read_main_table_archive(SELECTED, None),
            Err(EditError::PermissionDenied)
        ));
        assert_eq!(
            root(&client, SELECTED, 67, op, 1, 1, false),
            Err(EditError::PermissionDenied)
        );
        assert_eq!(
            group(&client, SELECTED, "g-90", 1, false),
            Err(EditError::PermissionDenied)
        );
        unchanged();
        f.f.connect().execute(sql, &[&SELECTED, &true]).unwrap();
    }
    for (id, restore, b, i, error) in [
        (62, false, 1, 1, EditError::Missing),
        (57, true, 1, 1, EditError::Missing),
        (68, true, 1, 1, EditError::InvalidDraft),
        (69, false, 1, 1, EditError::InvalidDraft),
        (67, true, 1, 1, EditError::InvalidDraft),
        (67, false, 2, 1, EditError::Conflict),
        (67, false, 1, 2, EditError::Conflict),
    ] {
        assert_eq!(root(&client, SELECTED, id, op, b, i, restore), Err(error));
        unchanged();
    }
    assert_eq!(
        root(&client, SELECTED, 67, Uuid::nil(), 1, 1, false),
        Err(EditError::InvalidDraft)
    );
    unchanged();
    for (id, restore, error) in [
        ("g-70", false, EditError::Missing),
        ("missing", true, EditError::Missing),
        ("g-90", true, EditError::InvalidDraft),
        ("g-30", false, EditError::InvalidDraft),
    ] {
        assert_eq!(group(&client, SELECTED, id, 1, restore), Err(error));
        unchanged();
    }
    f.f.connect().batch_execute("CREATE FUNCTION refuse_archive() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_archive AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_archive()").unwrap();
    assert_eq!(
        root(&client, SELECTED, 67, op, 1, 1, false),
        Err(EditError::HostFault)
    );
    unchanged();
    f.f.connect().batch_execute("DROP TRIGGER refuse_archive ON group_reorder_receipts; CREATE CONSTRAINT TRIGGER refuse_archive_group AFTER UPDATE ON board_groups DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_archive()").unwrap();
    assert_eq!(
        group(&client, SELECTED, "g-90", 1, false),
        Err(EditError::HostFault)
    );
    unchanged();
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_archive_group ON board_groups")
        .unwrap();
    root(&client, SELECTED, 67, op, 1, 1, false).unwrap();
    let mut saved = initial.lifecycle_records(&f.f, f.member, "archive");
    root(&f.client(), SELECTED, 67, op, 1, 1, false).unwrap();
    assert_eq!(f.state(), saved);
    group(&client, SELECTED, "g-90", 2, false).unwrap();
    saved = saved.group_transition(&f.f, "g-90", false);
    let page = client.read_main_table_archive(SELECTED, None).unwrap();
    assert_eq!(page.revision(), 3);
    assert_eq!(
        page.groups().iter().map(|g| g.id()).collect::<Vec<_>>(),
        ["g-30", "g-90"]
    );
    assert_eq!(
        page.items()
            .iter()
            .map(|r| r.id().as_u128())
            .collect::<Vec<_>>(),
        [57, 73, 69, 61, 67, 77]
    );
    assert!(page.items().iter().all(|r| !r.is_restorable()));
    assert_eq!(
        root(&client, SELECTED, 67, Uuid::new_v4(), 3, 2, true),
        Err(EditError::Missing)
    );
    assert_eq!(f.state(), saved);
    assert_eq!(
        group(&client, SELECTED, "g-90", 2, true),
        Err(EditError::Conflict)
    );
    assert_eq!(f.state(), saved);
    f.role("member");
    assert_eq!(
        root(&client, SELECTED, 67, op, 1, 1, false),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        group(&client, SELECTED, "g-90", 3, true),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(f.state(), saved);
    f.role("owner");
    group(&client, SELECTED, "g-90", 3, true).unwrap();
    saved = saved.group_transition(&f.f, "g-90", true);
    let page = f.client().read_main_table_archive(SELECTED, None).unwrap();
    assert_eq!(
        page.items()
            .iter()
            .map(|r| (r.id().as_u128(), r.is_restorable()))
            .collect::<Vec<_>>(),
        [(57, false), (67, true)]
    );
    root(&client, SELECTED, 67, Uuid::new_v4(), 4, 2, true).unwrap();
    saved = saved.lifecycle_records(&f.f, f.member, "restore");
    assert_eq!(
        client
            .read_selected_board(SELECTED)
            .unwrap()
            .iter()
            .map(|r| r.id.as_u128())
            .collect::<Vec<_>>(),
        [59, 73, 61, 67, 77]
    );
    // Same board-local ID on a different board is not a substitute target.
    assert_eq!(
        group(&client, SIBLING, "g-90", 1, false),
        Err(EditError::Missing)
    );
    assert_eq!(f.state(), saved);
    f.f.connect().execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'stored-'||n,'Empty',100+n,false FROM generate_series(1,101)n", &[&SELECTED]).unwrap();
    saved = f.state();
    let first = client.read_main_table_archive(SELECTED, None).unwrap();
    assert_eq!(first.groups().len(), 100);
    assert_eq!(first.groups()[0].id(), "g-30");
    assert_eq!(first.next_group_offset(), Some(100));
    let last = f
        .client()
        .read_main_table_archive(SELECTED, Some((5, 100)))
        .unwrap();
    assert_eq!(
        last.groups().iter().map(|g| g.id()).collect::<Vec<_>>(),
        ["stored-100", "stored-101"]
    );
    assert_eq!(last.next_group_offset(), None);
    assert!(matches!(
        client.read_main_table_archive(SELECTED, Some((4, 100))),
        Err(EditError::Conflict)
    ));
    assert_eq!(f.state(), saved);
    f.selection.session.revoke(&f.f);
    assert!(matches!(
        client.read_main_table_archive(SELECTED, None),
        Err(EditError::SessionRejected)
    ));
    assert_eq!(
        root(&client, SELECTED, 67, op, 1, 1, false),
        Err(EditError::SessionRejected)
    );
    assert_eq!(
        group(&client, SELECTED, "g-90", 5, false),
        Err(EditError::SessionRejected)
    );
    assert_eq!(f.state(), saved);
}
