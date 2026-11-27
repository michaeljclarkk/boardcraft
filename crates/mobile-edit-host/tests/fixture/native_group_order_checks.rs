use crate::{
    browser_transport, group_edit, native_group_order,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_mobile_edit_client::{EditClient, EditError};
use serde_json::json;

pub fn current_owner() {
    let f = native_group_order::fixture();
    let c = f.client();
    let abc = ["g-old", "g-z", "g-end"].map(String::from);
    let bac = ["g-z", "g-old", "g-end"].map(String::from);
    let mut state = f.state();
    let call = |client: &EditClient, board, revision, ids: &[String]| {
        client.reorder_main_table_groups(board, revision, ids)
    };
    assert_eq!(
        call(&EditClient::new(f.selection.host.port), SELECTED, 7, &bac),
        Err(EditError::SessionRejected)
    );
    let scoped = EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap();
    assert_eq!(call(&scoped, SELECTED, 7, &bac), Err(EditError::Missing));
    assert_eq!(call(&c, HIDDEN, 7, &bac), Err(EditError::PermissionDenied));
    let path = format!("/navigation/boards/{SELECTED}/main-table/group-order");
    let send = |method, path: &str, body: &str| {
        browser_transport::send(
            f.selection.host.port,
            method,
            path,
            &format!("127.0.0.1:{}", f.selection.host.port),
            &[(
                "Authorization",
                &format!("Bearer {}", f.selection.token.encoded()),
            )],
            body,
        )
    };
    for (method, p) in [
        ("PUT", path.clone()),
        ("POST", path.replace("/main-table", "")),
        ("POST", path.replace("/navigation", "")),
    ] {
        assert_eq!(send(method, &p, "{}").status, 404);
    }
    for role in ["member", "viewer"] {
        f.role(role);
        assert_eq!(
            call(&c, SELECTED, 7, &bac),
            Err(EditError::PermissionDenied)
        );
        assert_eq!(f.state(), state);
    }
    // A creator label does not elevate a Member. Restore its original policy afterward.
    let creator: uuid::Uuid =
        f.f.connect()
            .query_one(
                "SELECT creator_id FROM board_policies WHERE board_id=$1",
                &[&SELECTED],
            )
            .unwrap()
            .get(0);
    f.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$2 WHERE board_id=$1",
            &[&SELECTED, &f.member],
        )
        .unwrap();
    f.role("member");
    assert_eq!(
        call(&c, SELECTED, 7, &bac),
        Err(EditError::PermissionDenied)
    );
    f.role("owner");
    f.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$2 WHERE board_id=$1",
            &[&SELECTED, &creator],
        )
        .unwrap();
    f.selection.membership(&f.f, false);
    assert_eq!(
        call(&c, SELECTED, 7, &bac),
        Err(EditError::PermissionDenied)
    );
    f.selection.membership(&f.f, true);
    f.f.connect()
        .execute(
            "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
            &[&SELECTED, &f.member],
        )
        .unwrap();
    assert_eq!(
        call(&c, SELECTED, 7, &bac),
        Err(EditError::PermissionDenied)
    );
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
        assert_eq!(
            call(&c, SELECTED, 7, &bac),
            Err(EditError::PermissionDenied)
        );
        f.f.connect().execute(sql, &[&SELECTED, &true]).unwrap();
    }
    for body in [
        json!({"board_revision":7}),
        json!({"board_revision":7,"group_ids":"Work"}),
        json!({"board_revision":7,"group_ids":abc,"group_revision":1}),
    ] {
        assert_eq!(send("POST", &path, &body.to_string()).status, 400);
    }
    for (revision, ids, error) in [
        (0, abc.to_vec(), EditError::InvalidDraft),
        (6, abc.to_vec(), EditError::Conflict),
        (7, abc[..2].to_vec(), EditError::Conflict),
        (7, vec!["g-z".into(); 3], EditError::Conflict),
    ] {
        assert_eq!(call(&c, SELECTED, revision, &ids), Err(error));
    }
    f.f.connect().execute("INSERT INTO board_groups(board_id,id,name,position,active,deleted) VALUES($1,'gone','Work',20,false,true)",&[&SELECTED]).unwrap();
    f.f.connect().execute("INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'g-z','Peer',8,true)",&[&SIBLING]).unwrap();
    state = f.state();
    for id in ["g-hidden", "gone", "g-peer", "unknown"] {
        assert_eq!(
            call(&c, SELECTED, 7, &["g-old".into(), "g-z".into(), id.into()]),
            Err(EditError::Conflict)
        );
    }
    assert_eq!(
        send(
            "POST",
            &path,
            &json!({"board_revision":7,"group_ids":vec!["g-z";501]}).to_string()
        )
        .status,
        413
    );
    assert_eq!(f.state(), state);
    f.f.connect().execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'extra-'||n,'Work',n,true FROM generate_series(1,498) n", &[&SELECTED]).unwrap();
    let large = f.state();
    assert_eq!(
        send(
            "POST",
            &path,
            &json!({"board_revision":7,"group_ids":abc}).to_string()
        )
        .status,
        413
    );
    assert_eq!(f.state(), large);
    f.f.connect()
        .execute(
            "DELETE FROM board_groups WHERE board_id=$1 AND id LIKE 'extra-%'",
            &[&SELECTED],
        )
        .unwrap();
    f.f.connect().batch_execute(group_edit::FAULT).unwrap();
    assert_eq!(call(&c, SELECTED, 7, &bac), Err(EditError::HostFault));
    assert_eq!(f.state(), state); // Reconnect before removing the deferred fault.
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    call(&c, SELECTED, 7, &bac).unwrap();
    state = state.reordered_groups(&f.f, &["g-z", "g-old", "g-end"]);
    assert_eq!(call(&c, SELECTED, 7, &bac), Err(EditError::Conflict));
    call(&c, SELECTED, 8, &bac).unwrap(); // Direct unchanged permutation is still an explicit save.
    state = state.reordered_groups(&f.f, &["g-z", "g-old", "g-end"]);
    let read = f.client().read_main_table(SELECTED).unwrap();
    assert_eq!(read.revision(), 9);
    assert_eq!(
        read.groups()
            .iter()
            .map(|g| (g.id(), g.items().len(), g.revision(), g.is_collapsed()))
            .collect::<Vec<_>>(),
        [
            ("g-z", 2, 3, true),
            ("g-old", 1, 3, false),
            ("g-end", 0, 3, false)
        ]
    );
    f.selection.session.revoke(&f.f);
    assert_eq!(call(&c, SELECTED, 9, &abc), Err(EditError::SessionRejected));
    assert_eq!(f.state(), state);
}
