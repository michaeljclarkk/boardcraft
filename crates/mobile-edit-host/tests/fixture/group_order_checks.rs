use crate::{
    create::{Case, login, rejected},
    group_edit::FAULT,
    group_edit_checks::{deny_without_owner_grants, owner},
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};

fn path() -> String {
    format!("/preview/api/navigation/boards/{SELECTED}/group-order")
}
fn order(revision: i64, ids: &[&str]) -> Value {
    json!({"board_revision":revision,"group_ids":ids})
}
const ABC: [&str; 3] = ["g-old", "g-z", "g-end"];

pub fn access() {
    let c = owner();
    let p = path();
    let d = order(7, &ABC);
    rejected(c.request(None, "POST", &p, &d), 401);
    for (method, route, status) in [
        ("PUT", p.clone(), 404),
        ("POST", p.replace("/navigation/boards", "/boards"), 404),
        ("POST", p.replace("/preview/api", ""), 403),
        (
            "POST",
            p.replace(&SELECTED.to_string(), &HIDDEN.to_string()),
            403,
        ),
    ] {
        rejected(c.request(Some(&c.pair), method, &route, &d), status);
    }
    let snapshot = Snapshot::read(&c.f);
    deny_without_owner_grants(&c, &snapshot, || c.request(Some(&c.pair), "POST", &p, &d));
}

pub fn permutations() {
    let c = owner();
    let mut db = c.f.connect();
    let p = path();
    db.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'g-z','Work',9,true)",
        &[&SIBLING],
    )
    .unwrap();
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active,deleted) VALUES($1,'gone','Work',9,false,true)",&[&SELECTED]).unwrap();
    let mut s = Snapshot::read(&c.f);
    let post = |d: &Value| c.request(Some(&c.pair), "POST", &p, d);
    for (d, status) in [
        (json!({"board_revision":7}), 400),
        (json!({"board_revision":7,"group_ids":"Work"}), 400),
        (
            json!({"board_revision":7,"group_ids":ABC,"group_revision":1}),
            400,
        ),
        (order(0, &ABC), 422),
        (order(6, &ABC), 409),
        (order(7, &ABC[..2]), 409),
        (order(7, &["g-z", "g-z", "g-end"]), 409),
        (order(7, &vec!["g-z"; 501]), 413),
    ] {
        rejected(post(&d), status);
    }
    for id in ["g-hidden", "gone", "g-peer", "unknown"] {
        rejected(post(&order(7, &["g-old", "g-z", id])), 409);
    }
    assert_eq!(Snapshot::read(&c.f), s);
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'extra-'||n,'Work',n,true FROM generate_series(1,498) n",&[&SELECTED]).unwrap();
    let large = Snapshot::read(&c.f);
    rejected(post(&order(7, &ABC)), 413);
    assert_eq!(Snapshot::read(&c.f), large);
    db.execute(
        "DELETE FROM board_groups WHERE board_id=$1 AND id LIKE 'extra-%'",
        &[&SELECTED],
    )
    .unwrap();
    assert_eq!(post(&order(7, &ABC)).status, 200);
    s = s.reordered_groups(&c.f, &ABC);
    let a = ["g-z", "g-old", "g-end"];
    let b = ["g-old", "g-end", "g-z"];
    db.batch_execute(FAULT).unwrap();
    rejected(post(&order(8, &a)), 500);
    assert_eq!(Snapshot::read(&c.f), s);
    db.batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    let peer = Host::start_preview(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let pair = login(peer.port, "reader", &c.password);
    let barrier = std::sync::Barrier::new(2);
    let statuses = std::thread::scope(|scope| {
        let other = scope.spawn(|| {
            barrier.wait();
            Case::request_at(peer.port, Some(&pair), "POST", &p, &order(8, &a)).status
        });
        barrier.wait();
        [post(&order(8, &b)).status, other.join().unwrap()]
    });
    let winner = match statuses {
        [200, 409] => b,
        [409, 200] => a,
        _ => panic!("unexpected race: {statuses:?}"),
    };
    s = s.reordered_groups(&c.f, &winner);
    rejected(post(&order(8, &winner)), 409);
    assert_eq!(Snapshot::read(&c.f), s);
}
