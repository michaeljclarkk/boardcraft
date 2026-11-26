use crate::{
    browser_transport::{Reply, send},
    fixture::Fixture,
    selection::{self, Catalog, SELECTED},
    stored_session::StoredSession,
};
use boardcraft_account::Role;
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_client::EditClient;
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

struct BrowserPair {
    cookie: String,
    binding: String,
}

fn login(port: u16, identifier: &str, password: &str) -> BrowserPair {
    let authority = format!("127.0.0.1:{port}");
    let reply = send(
        port,
        "POST",
        "/preview/api/session",
        &authority,
        &[
            ("Origin", &format!("http://{authority}")),
            ("Boardcraft-Preview", "login"),
        ],
        &json!({"identifier":identifier,"password":password}).to_string(),
    );
    assert_eq!(reply.status, 200);
    BrowserPair {
        cookie: reply.cookie.unwrap().split(';').next().unwrap().to_string(),
        binding: serde_json::from_str(&reply.body).unwrap(),
    }
}

struct Case {
    host: Host,
    f: Fixture,
    member: Uuid,
    catalog: Catalog,
    password: String,
    session: StoredSession,
    pair: BrowserPair,
}

impl Case {
    fn new() -> Self {
        let f = Fixture::start();
        let password = Uuid::new_v4().to_string();
        let member = f.seed_account("reader", &password, Role::Member);
        let catalog = Catalog::new(&f, member);
        selection::seed_groups(&f);
        let host = Host::start_preview(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
        let pair = login(host.port, "reader", &password);
        let session = StoredSession::only(&f, member);
        Self {
            host,
            f,
            member,
            catalog,
            password,
            session,
            pair,
        }
    }

    fn request(&self, pair: Option<&BrowserPair>, method: &str, path: &str, body: &Value) -> Reply {
        let authority = format!("127.0.0.1:{}", self.host.port);
        let origin = format!("http://{authority}");
        let mut fields = vec![("Origin", origin.as_str())];
        if let Some(p) = pair {
            fields.extend([
                ("Boardcraft-Preview", p.binding.as_str()),
                ("Cookie", p.cookie.as_str()),
            ]);
        }
        send(
            self.host.port,
            method,
            path,
            &authority,
            &fields,
            &body.to_string(),
        )
    }

    fn create(&self, body: &Value) -> Reply {
        self.request(Some(&self.pair), "POST", &path(), body)
    }

    fn check(&self, joined: bool, live: bool, appended: Option<(Uuid, &str)>) {
        assert!(self.session.present(&self.f) == live);
        self.catalog.check_grants(&self.f, joined);
        selection::check_group_items(
            &self.f,
            false,
            appended.map(|(id, title)| (id, "g-10", 0, title, self.member)),
        );
    }
}

fn path() -> String {
    format!("/preview/api/navigation/boards/{SELECTED}/items")
}
fn draft(id: Uuid, group: &str, title: &str) -> Value {
    json!({"id":id,"group_id":group,"title":title})
}
fn rejected(reply: Reply, status: u16) {
    assert_eq!(reply.status, status);
    assert!(reply.body.is_empty());
}

pub fn access() {
    let c = Case::new();
    let id = Uuid::new_v4();
    let valid = draft(id, "g-20", "Ink");
    rejected(c.request(None, "POST", &path(), &valid), 401);
    c.check(true, true, None);
    c.catalog.membership(&c.f, false);
    rejected(c.create(&valid), 403);
    c.check(false, true, None);
    c.catalog.membership(&c.f, true);
    // Retire a formerly active group before submitting its old form.
    let archive = |active: bool| {
        assert_eq!(
            c.f.connect()
                .execute(
                    "UPDATE board_groups SET active=$2 WHERE board_id=$1 AND id='g-20'",
                    &[&SELECTED, &active]
                )
                .unwrap(),
            1
        )
    };
    archive(false);
    rejected(c.create(&valid), 422);
    assert!(
        c.f.connect()
            .query_opt("SELECT id FROM board_items WHERE id=$1", &[&id])
            .unwrap()
            .is_none()
    );
    archive(true);
    c.check(true, true, None);
    for group in ["missing", "g-70"] {
        rejected(c.create(&draft(id, group, "Ink")), 422);
        c.check(true, true, None);
    }
    let nav =
        Host::start_with_navigation(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    for port in [nav.port, c.f.host.port, c.host.port] {
        let authority = format!("127.0.0.1:{port}");
        for route in [
            format!("/navigation/boards/{SELECTED}/items"),
            format!("/boards/{}/items", c.f.board),
        ] {
            rejected(
                send(port, "POST", &route, &authority, &[], &valid.to_string()),
                404,
            );
            c.check(true, true, None);
        }
        if port != c.host.port {
            rejected(
                send(port, "POST", &path(), &authority, &[], &valid.to_string()),
                404,
            );
            c.check(true, true, None);
        }
    }
    c.session.revoke(&c.f);
    rejected(c.create(&valid), 401);
    c.check(true, false, None);
}

pub fn atomic() {
    let c = Case::new();
    let id = Uuid::new_v4();
    for invalid in [
        draft(id, "g-10", "  "),
        draft(Uuid::nil(), "g-10", "Ink"),
        draft(id, "g-10", &"😀".repeat(128)),
    ] {
        rejected(c.create(&invalid), 422);
        c.check(true, true, None);
    }
    for key in ["actor", "can_edit", "position", "revision"] {
        let mut invalid = draft(id, "g-10", "Ink");
        invalid[key] = json!(1);
        rejected(c.create(&invalid), 400);
        c.check(true, true, None);
    }
    let title = format!("{}a", "😀".repeat(127));
    let body = draft(id, "g-10", &format!(" {title} "));
    let appended = Some((id, title.as_str()));
    for _ in 0..2 {
        let reply = c.create(&body);
        assert_eq!(reply.status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&reply.body).unwrap(),
            json!({"id":id,"group_id":"g-10","title":title,"revision":1})
        );
        c.check(true, true, appended);
    }
    for collision in [
        draft(id, "g-10", "Other"),
        draft(id, "g-20", &title),
        draft(Uuid::from_u128(59), "g-20", "Ink"),
        draft(Uuid::from_u128(62), "g-20", "Outline"),
    ] {
        rejected(c.create(&collision), 409);
        c.check(true, true, appended);
    }
    let peer_password = Uuid::new_v4().to_string();
    let peer = c.f.seed_account("peer", &peer_password, Role::Member);
    let mut db = c.f.connect();
    db.execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM board_policies WHERE board_id=$1", &[&SELECTED,&peer]).unwrap();
    db.execute(
        "INSERT INTO board_subscribers VALUES ($1,$2)",
        &[&SELECTED, &peer],
    )
    .unwrap();
    let peer_pair = login(c.host.port, "peer", &peer_password);
    rejected(c.request(Some(&peer_pair), "POST", &path(), &body), 409);
    c.check(true, true, appended);

    db.batch_execute("CREATE FUNCTION reject_append() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit fault'; END $$;
        CREATE CONSTRAINT TRIGGER fail_commit AFTER INSERT ON board_items DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_append()").unwrap();
    let fresh = Uuid::new_v4();
    rejected(c.create(&draft(fresh, "g-20", "Ink")), 500);
    c.check(true, true, appended);
    assert!(
        c.f.connect()
            .query_opt("SELECT id FROM board_items WHERE id=$1", &[&fresh])
            .unwrap()
            .is_none()
    );
    db.batch_execute("DROP TRIGGER fail_commit ON board_items; DROP FUNCTION reject_append()")
        .unwrap();

    let token = boardcraft_account::sign_in(&c.f.schema.config, "reader", &c.password).unwrap();
    let client = EditClient::with_bearer(c.host.port, token.encoded()).unwrap();
    let flat = serde_json::to_value(client.read_selected_board(SELECTED).unwrap()).unwrap();
    let reply = c.request(
        Some(&c.pair),
        "GET",
        &format!("/preview/api/navigation/boards/{SELECTED}"),
        &Value::Null,
    );
    assert_eq!(reply.status, 200);
    let groups: Value = serde_json::from_str(&reply.body).unwrap();
    let row = |id: Uuid, title: &str| json!({"id":id,"title":title,"revision":1});
    let expected = json!([
        row(Uuid::from_u128(73), "Sketch"),
        row(Uuid::from_u128(61), "Ink"),
        row(id, &title),
        row(Uuid::from_u128(59), "Ink")
    ]);
    assert_eq!(flat, expected);
    assert_eq!(groups[1]["id"], "g-10");
    assert_eq!(groups[1]["items"], json!([row(id, &title)]));
    assert_eq!(
        groups
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["items"].as_array().unwrap())
            .collect::<Vec<_>>(),
        expected.as_array().unwrap().iter().collect::<Vec<_>>()
    );
    c.check(true, true, appended);
}
