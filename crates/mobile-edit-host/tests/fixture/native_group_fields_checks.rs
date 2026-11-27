use crate::{
    group_create, group_edit, native_group_fields,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_mobile_edit_client::{EditClient, EditError, GroupFields};
use uuid::Uuid;

pub fn current_owner() {
    let f = native_group_fields::fixture();
    let c = f.client();
    let id = format!("g-{}", Uuid::new_v4());
    let fields = GroupFields::new(7, "  Work  ", "#AbcDEF");
    let mut state = f.state();
    let creator: Uuid =
        f.f.connect()
            .query_one(
                "SELECT creator_id FROM board_policies WHERE board_id=$1",
                &[&SELECTED],
            )
            .unwrap()
            .get(0);
    let both = |c: &EditClient, board, error| {
        assert!(matches!(c.create_main_table_group(board, &id, &fields), Err(e) if e == error));
        assert_eq!(
            c.edit_main_table_group(board, "g-z", 1, &fields),
            Err(error)
        );
        assert_eq!(f.state(), state);
    };
    both(
        &EditClient::new(f.selection.host.port),
        SELECTED,
        EditError::SessionRejected,
    );
    both(
        &EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap(),
        SELECTED,
        EditError::Missing,
    );
    both(&c, HIDDEN, EditError::PermissionDenied);
    // The field commands do not expose a generic structural API.
    for (method, suffix) in [
        ("POST", "main-table/groups/g-z"),
        ("PUT", "main-table/groups"),
        ("POST", "groups"),
        ("PUT", "groups/g-z"),
    ] {
        let response = crate::browser_transport::send(
            f.selection.host.port,
            method,
            &format!("/navigation/boards/{SELECTED}/{suffix}"),
            &format!("127.0.0.1:{}", f.selection.host.port),
            &[(
                "Authorization",
                &format!("Bearer {}", f.selection.token.encoded()),
            )],
            "{}",
        );
        assert_eq!(response.status, 404);
        assert_eq!(f.state(), state);
    }
    for role in ["member", "viewer"] {
        f.role(role);
        both(&c, SELECTED, EditError::PermissionDenied);
        f.check_access(role, true, true);
    }
    f.role("owner");
    // Being the board creator does not elevate a Member to Owner.
    f.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$1 WHERE board_id=$2",
            &[&f.member, &SELECTED],
        )
        .unwrap();
    f.role("member");
    both(&c, SELECTED, EditError::PermissionDenied);
    f.role("owner");
    f.selection.membership(&f.f, false);
    both(&c, SELECTED, EditError::PermissionDenied);
    f.selection.membership(&f.f, true);
    f.f.connect()
        .execute(
            "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
            &[&SELECTED, &f.member],
        )
        .unwrap();
    // Remove the creator exception as well as the subscriber grant.
    f.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$2 WHERE board_id=$1",
            &[&SELECTED, &creator],
        )
        .unwrap();
    both(&c, SELECTED, EditError::PermissionDenied);
    f.f.connect()
        .execute(
            "INSERT INTO board_subscribers VALUES($1,$2)",
            &[&SELECTED, &f.member],
        )
        .unwrap();
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        f.f.connect().execute(sql, &[&SELECTED, &false]).unwrap();
        both(&c, SELECTED, EditError::PermissionDenied);
        f.f.connect().execute(sql, &[&SELECTED, &true]).unwrap();
    }
    for target in ["g-peer", "absent", "g-hidden"] {
        assert_eq!(
            c.edit_main_table_group(SELECTED, target, 1, &fields),
            Err(EditError::Missing)
        );
        assert_eq!(f.state(), state);
    }
    assert_eq!(
        c.edit_main_table_group(SIBLING, "g-z", 1, &GroupFields::new(1, "Work", "#443322")),
        Err(EditError::Missing)
    );
    for (b, g, name, colour, error) in [
        (0, 1, "Work", "#443322", EditError::InvalidDraft),
        (7, 0, "Work", "#443322", EditError::InvalidDraft),
        (6, 1, "Work", "#443322", EditError::Conflict),
        (7, 2, "Work", "#443322", EditError::Conflict),
        (7, 1, "  ", "#443322", EditError::InvalidDraft),
        (7, 1, "Work", "#12345z", EditError::InvalidDraft),
    ] {
        assert_eq!(
            c.edit_main_table_group(SELECTED, "g-z", g, &GroupFields::new(b, name, colour)),
            Err(error)
        );
        assert_eq!(f.state(), state);
    }
    assert_eq!(
        c.create_main_table_group(SELECTED, "g-invalid", &fields),
        Err(EditError::InvalidDraft)
    );
    assert_eq!(
        c.create_main_table_group(SELECTED, &id, &GroupFields::new(6, "Work", "#443322")),
        Err(EditError::Conflict)
    );
    f.f.connect().batch_execute(group_create::FAULT).unwrap();
    assert_eq!(
        c.create_main_table_group(SELECTED, &id, &fields),
        Err(EditError::HostFault)
    );
    assert_eq!(f.state(), state);
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_group ON board_groups")
        .unwrap();
    c.create_main_table_group(SELECTED, &id, &fields).unwrap();
    state = state.created_group(&f.f, &id, "Work", "#AbcDEF", 3);
    c.create_main_table_group(SELECTED, &id, &GroupFields::new(7, "Other", "#123456"))
        .unwrap();
    assert_eq!(f.state(), state);
    f.f.connect().batch_execute(group_edit::FAULT).unwrap();
    assert_eq!(
        c.edit_main_table_group(
            SELECTED,
            "g-z",
            1,
            &GroupFields::new(8, "  Reserve  ", "#00c875")
        ),
        Err(EditError::HostFault)
    );
    assert_eq!(f.state(), state);
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    // Unchanged explicit Save advances both baselines and only those records.
    c.edit_main_table_group(SELECTED, "g-z", 1, &GroupFields::new(8, "Work", "#443322"))
        .unwrap();
    state = state.edited_group(&f.f, "g-z", "Work", "#443322");
    assert_eq!(
        c.edit_main_table_group(SELECTED, "g-z", 1, &GroupFields::new(8, "Work", "#443322")),
        Err(EditError::Conflict)
    );
    let read = f.client().read_main_table(SELECTED).unwrap();
    assert_eq!(read.revision(), 9);
    assert_eq!(
        read.groups().iter().map(|g| g.id()).collect::<Vec<_>>(),
        [&id, "g-old", "g-z", "g-end"]
    );
    let g = read.groups().iter().find(|g| g.id() == "g-z").unwrap();
    assert_eq!((g.name(), g.colour(), g.revision()), ("Work", "#443322", 2));
    assert_eq!(
        g.items().iter().map(|r| r.id.as_u128()).collect::<Vec<_>>(),
        [61, 63]
    );
    f.role("member");
    assert_eq!(
        c.create_main_table_group(SELECTED, &id, &fields),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(f.state(), state);
    f.role("owner");
    f.selection.session.revoke(&f.f);
    assert_eq!(
        c.create_main_table_group(SELECTED, &id, &fields),
        Err(EditError::SessionRejected)
    );
    assert_eq!(
        c.edit_main_table_group(SELECTED, "g-z", 2, &GroupFields::new(9, "Work", "#443322")),
        Err(EditError::SessionRejected)
    );
    assert_eq!(f.state(), state);
    f.check_access("owner", true, false);
}
