use boardcraft_board::store::SaveError;
use boardcraft_board::{Board, BoardId, ItemId, RenameError, automation, store};
use postgres::{Client, Config, NoTls, error::SqlState};
use uuid::Uuid;

fn test_config() -> Config {
    let url = std::env::var("RUSTDAY_TEST_DATABASE_URL")
        .expect("RUSTDAY_TEST_DATABASE_URL must name a disposable boardcraft test database");
    let config: Config = url.parse().expect("invalid PostgreSQL connection settings");
    assert!(
        config.get_dbname() == Some("rustday_test"),
        "expected explicit rustday_test database"
    );
    config
}

fn reconnect(config: &Config, schema: &str) -> Client {
    let mut client = config.connect(NoTls).unwrap();
    client
        .batch_execute(&format!("SET search_path TO {schema}"))
        .unwrap();
    client
}

#[test]
fn reconnect_after_save_rejection_and_failed_commit() {
    let config = test_config();
    let schema = format!("board_test_{}", Uuid::new_v4().simple());
    let mut db = config.connect(NoTls).unwrap();
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    store::migrate(&mut db).unwrap();

    let board = BoardId::new(Uuid::from_u128(5)).unwrap();
    let other = BoardId::new(Uuid::from_u128(6)).unwrap();
    let first = ItemId::new(Uuid::from_u128(42)).unwrap();
    let second = ItemId::new(Uuid::from_u128(17)).unwrap();
    let foreign = ItemId::new(Uuid::from_u128(30)).unwrap();
    let mut expected = Board::new(board);
    expected.add_item(first, "Outline").unwrap();
    expected.add_item(second, "Outline").unwrap();
    let mut untouched = Board::new(other);
    untouched
        .add_item(foreign, "  Preserve spelling  ")
        .unwrap();
    store::create_board(&mut db, &expected).unwrap();
    store::create_board(&mut db, &untouched).unwrap();

    let max_title = format!("{}!", "🪴".repeat(127)); // 255 UTF-16 units
    let draft = format!("\t{max_title}  ");
    assert_eq!(
        store::rename_item(&mut db, board, second, &draft).unwrap(),
        max_title
    );
    expected.rename_item(second, &max_title).unwrap();
    drop(db);

    let mut db = reconnect(&config, &schema);
    store::migrate(&mut db).unwrap(); // reopening a populated schema is safe
    assert_eq!(
        store::load_board(&mut db, board).unwrap().unwrap(),
        expected
    );
    for (item, draft, error) in [
        (second, " \t\n".to_owned(), RenameError::BlankTitle),
        (second, "🪴".repeat(128), RenameError::TitleTooLong),
        (
            foreign,
            "Wrong board".to_owned(),
            RenameError::MissingItem(foreign),
        ),
    ] {
        assert!(matches!(store::rename_item(&mut db, board, item, &draft),
            Err(SaveError::Rejected(actual)) if actual == error));
    }

    // Test-only constraint: UPDATE ... RETURNING succeeds, but COMMIT fails.
    db.batch_execute(
        "ALTER TABLE board_items ADD UNIQUE (board_id, title) DEFERRABLE INITIALLY DEFERRED",
    )
    .unwrap();
    assert!(
        matches!(store::rename_item(&mut db, board, second, "Outline"),
        Err(SaveError::Database(e)) if e.code() == Some(&SqlState::UNIQUE_VIOLATION))
    );
    drop(db);

    let mut db = reconnect(&config, &schema);
    assert_eq!(
        store::load_board(&mut db, board).unwrap().unwrap(),
        expected
    );
    assert_eq!(
        store::load_board(&mut db, other).unwrap().unwrap(),
        untouched
    );
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

#[test]
fn automation_rule_selection_and_run_are_transactional() {
    use automation::{
        Action, ActionKind, Condition, ConditionMode, ConditionOperator, Definition, Event,
        Outcome, Recipient, SaveInput, Trigger, TriggerKind,
    };
    let config = test_config();
    let schema = format!("automation_test_{}", Uuid::new_v4().simple());
    let mut db = config.connect(NoTls).unwrap();
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    store::migrate(&mut db).unwrap();
    db.batch_execute(
        "CREATE TABLE accounts(id uuid PRIMARY KEY, username text NOT NULL,
         email text NOT NULL, role text NOT NULL, active boolean NOT NULL)",
    )
    .unwrap();
    let board = BoardId::new(Uuid::from_u128(7)).unwrap();
    let item = ItemId::new(Uuid::from_u128(8)).unwrap();
    let actor = Uuid::from_u128(9);
    let mut seed = Board::new(board);
    seed.add_item(item, "Ink").unwrap();
    store::create_board(&mut db, &seed).unwrap();
    db.execute(
        "INSERT INTO accounts(id,username,email,role,active) VALUES($1,'Owner','owner@example.test','owner',true)",
        &[&actor],
    )
    .unwrap();
    let catalog = automation::catalog(&mut db, board.as_uuid()).unwrap();
    assert!(catalog.column("Name").is_some());
    let status = catalog
        .columns
        .iter()
        .find(|column| column.kind == "status")
        .unwrap()
        .clone();
    let definition = Definition {
        trigger: Trigger {
            kind: TriggerKind::ColumnChanged,
            column_id: Some("Name".into()),
            value: None,
            previous_value: None,
        },
        conditions: vec![Condition {
            column_id: status.id.clone(),
            operator: ConditionOperator::IsNot,
            value: Some("done".into()),
        }],
        actions: vec![Action {
            kind: ActionKind::Notify,
            column_id: None,
            value: None,
            person_ids: Vec::new(),
            days: 0,
            recipient: Some(Recipient::Creator),
            title: Some("Changed".into()),
            message: Some("{item.name} changed".into()),
            date_basis: None,
        }],
        conditions_mode: ConditionMode::All,
        action_groups: Vec::new(),
    };
    let input = SaveInput {
        id: format!("custom-{}", Uuid::from_u128(10).simple()),
        expected_revision: 0,
        name: "Name changes".into(),
        enabled: true,
        definition,
    };
    let saved = automation::save(&mut db, board.as_uuid(), &input, actor).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.trigger, "When Name changes");
    assert!(saved.enabled);

    let mut tx = db.transaction().unwrap();
    let event = Event::column_changed(
        actor,
        "Name",
        Some("Ink".into()),
        Some("Ink 2".into()),
        Some("rename:1".into()),
    );
    assert_eq!(automation::apply(&mut tx, board, item, event).unwrap(), 1);
    tx.commit().unwrap();

    // Replaying the same accepted transition must not queue a second effect.
    let mut tx = db.transaction().unwrap();
    let event = Event::column_changed(
        actor,
        "Name",
        Some("Ink".into()),
        Some("Ink 2".into()),
        Some("rename:1".into()),
    );
    assert_eq!(automation::apply(&mut tx, board, item, event).unwrap(), 0);
    tx.commit().unwrap();
    let runs = automation::history(&mut db, board.as_uuid(), None, 0).unwrap();
    assert_eq!(runs.runs.len(), 1);
    assert_eq!(runs.runs[0].outcome, Outcome::Succeeded);
    let inbox: i64 = db
        .query_one("SELECT count(*) FROM inbox_notifications", &[])
        .unwrap()
        .get(0);
    assert_eq!(inbox, 1);

    // A condition that is not met records the run but applies no action.
    db.execute(
        "INSERT INTO status_cells(board_id,column_id,item_id,value,revision)
         VALUES($1,$2,$3,'done',1)",
        &[
            &board.as_uuid(),
            &Uuid::parse_str(&status.id).unwrap(),
            &item.as_uuid(),
        ],
    )
    .unwrap();
    let mut tx = db.transaction().unwrap();
    let event = Event::column_changed(
        actor,
        "Name",
        Some("Ink 2".into()),
        Some("Ink 3".into()),
        Some("rename:2".into()),
    );
    assert_eq!(automation::apply(&mut tx, board, item, event).unwrap(), 1);
    tx.commit().unwrap();
    let runs = automation::history(&mut db, board.as_uuid(), None, 0).unwrap();
    assert_eq!(runs.runs[0].outcome, Outcome::ConditionNotMet);
    let inbox: i64 = db
        .query_one("SELECT count(*) FROM inbox_notifications", &[])
        .unwrap()
        .get(0);
    assert_eq!(inbox, 1);

    // Revision conflicts protect concurrent edits; archiving disables the rule.
    assert!(matches!(
        automation::set_enabled(
            &mut db,
            board.as_uuid(),
            Uuid::from_u128(10),
            0,
            false,
            actor
        ),
        Err(automation::StoreError::Conflict)
    ));
    let archived =
        automation::set_archived(&mut db, board.as_uuid(), Uuid::from_u128(10), 1, true).unwrap();
    assert!(archived.archived && !archived.enabled);
    assert_eq!(archived.revision, 2);
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

#[test]
fn automation_migration_preserves_the_first_slice_rule_and_run() {
    let config = test_config();
    let schema = format!("automation_migration_{}", Uuid::new_v4().simple());
    let mut db = config.connect(NoTls).unwrap();
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    for migration in [
        include_str!("../migrations/0001_boards.sql"),
        include_str!("../migrations/0003_board_groups.sql"),
        include_str!("../migrations/0015_automations.sql"),
        include_str!("../migrations/0016_inbox_notifications.sql"),
    ] {
        db.batch_execute(migration).unwrap();
    }
    let board = Uuid::from_u128(31);
    let item = Uuid::from_u128(32);
    let rule = Uuid::from_u128(33);
    let actor = Uuid::from_u128(34);
    db.execute("INSERT INTO boards(id) VALUES($1)", &[&board])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'main','Items',0,true)",
        &[&board],
    )
    .unwrap();
    db.execute(
        "INSERT INTO board_items(id,board_id,group_id,title,position) VALUES($1,$2,'main','Ink',0)",
        &[&item, &board],
    )
    .unwrap();
    db.execute(
        "INSERT INTO automations(id,board_id,name,trigger,action,enabled,created_by) VALUES($1,$2,'Audit','item_renamed','record',true,$3)",
        &[&rule, &board, &actor],
    )
    .unwrap();
    db.execute(
        "INSERT INTO automation_runs(id,automation_id,board_id,item_id,item_title) VALUES($1,$2,$3,$4,'Ink 2')",
        &[&Uuid::from_u128(35), &rule, &board, &item],
    )
    .unwrap();
    db.batch_execute(include_str!("../migrations/0019_automation_engine.sql"))
        .unwrap();
    let row = db
        .query_one(
            "SELECT name,definition->'trigger'->>'kind',revision,archived,actions FROM automations WHERE id=$1",
            &[&rule],
        )
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "Audit");
    assert_eq!(row.get::<_, String>(1), "column_changed");
    assert_eq!(row.get::<_, i32>(2), 1);
    assert!(!row.get::<_, bool>(3));
    assert_eq!(
        row.get::<_, Vec<String>>(4),
        ["Notify Creator: Item {item.name} was renamed."]
    );
    let run = db
        .query_one(
            "SELECT idempotency_key,rule_name,outcome,actor_id FROM automation_runs WHERE automation_id=$1",
            &[&rule],
        )
        .unwrap();
    assert_eq!(
        run.get::<_, String>(0),
        format!("legacy:{}", Uuid::from_u128(35))
    );
    assert_eq!(run.get::<_, String>(1), "Audit");
    assert_eq!(run.get::<_, String>(2), "succeeded");
    assert_eq!(run.get::<_, Uuid>(3), actor);
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}
