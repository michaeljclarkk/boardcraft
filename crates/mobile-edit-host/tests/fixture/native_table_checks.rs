use crate::{
    native_table::{A, B, C, NativeTable},
    selection::{SELECTED, SIBLING},
};
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit};
use uuid::Uuid;

pub fn current_access() {
    let f = NativeTable::new();
    let client = f.client();
    let mut state = f.state();
    f.check_read(1, &[], true);
    assert_eq!(client.read_main_table(SELECTED).unwrap().revision(), 7);
    let ids = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
    client
        .set_main_table_fold(SELECTED, 1, &ids(&[B]), true)
        .unwrap();
    state = f.folded(&state, 2, &[B]);
    f.check_read(2, &[B], true);
    assert_eq!(
        client.set_main_table_fold(SELECTED, 1, &ids(&[A]), true),
        Err(EditError::Conflict)
    );
    assert_eq!(
        client.set_main_table_fold(SELECTED, 2, &ids(&[A, "g-peer"]), true),
        Err(EditError::Missing)
    );
    f.role("viewer");
    f.check_read(2, &[B], false);
    assert_eq!(
        client.set_main_table_fold(SELECTED, 2, &ids(&[B]), false),
        Err(EditError::PermissionDenied)
    );
    f.role("owner");
    f.check_read(2, &[B], true);
    client
        .set_main_table_fold(SELECTED, 2, &ids(&[A, B, C]), true)
        .unwrap();
    state = f.folded(&state, 3, &[A, B, C]);
    f.selection.membership(&f.f, false);
    assert!(matches!(
        client.read_main_table(SELECTED),
        Err(EditError::PermissionDenied)
    ));
    assert_eq!(
        client.set_main_table_fold(SELECTED, 3, &ids(&[B]), false),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(f.state(), state);
    f.selection.membership(&f.f, true);
    f.role("member");
    f.f.connect().batch_execute("CREATE FUNCTION refuse_fold() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_fold AFTER UPDATE ON main_tables DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_fold()").unwrap();
    assert_eq!(
        client.set_main_table_fold(SELECTED, 3, &ids(&[B]), false),
        Err(EditError::HostFault)
    );
    assert_eq!(f.state(), state);
    f.f.connect()
        .batch_execute("DROP TRIGGER refuse_fold ON main_tables")
        .unwrap();
    let direct = EditClient::with_bearer(f.f.host.port, f.selection.token.encoded()).unwrap();
    assert!(matches!(
        direct.read_main_table(SELECTED),
        Err(EditError::Missing)
    ));
    assert_eq!(
        direct.set_main_table_fold(SELECTED, 3, &ids(&[B]), false),
        Err(EditError::Missing)
    );
    assert_eq!(client.read(SELECTED), Err(EditError::Missing));
    assert_eq!(
        client.rename(
            SIBLING,
            Uuid::from_u128(62),
            &NameEdit {
                title: "Coral".into(),
                revision: 1
            }
        ),
        Err(EditError::Missing)
    );
    assert_eq!(direct.read(f.f.board).unwrap().len(), 2);
    f.selection.session.revoke(&f.f);
    assert!(matches!(
        client.read_main_table(SELECTED),
        Err(EditError::SessionRejected)
    ));
    assert_eq!(
        client.set_main_table_fold(SELECTED, 3, &ids(&[B]), false),
        Err(EditError::SessionRejected)
    );
    assert_eq!(f.state(), state);
}
