use super::{
    fixture::Fixture,
    selection,
    stored_session::{StoredSession, assert_no_session},
};
use boardcraft_account::{Role, Token};
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_client::EditClient;
use boardcraft_mobile_edit_host::Host;
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpStream},
    time::Duration,
};

// Raw HTTP permits exercising duplicate fields. Extract just the cookie and
// login result in process. No response/request is formatted on failure.
pub(super) struct Reply {
    pub(super) status: u16,
    access: Option<String>,
    pub(super) cookie: Option<String>,
    pub(super) body: String,
}
pub(super) fn send(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    fields: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
    write!(s,"{method} {path} HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",body.len()).unwrap();
    for (k, v) in fields {
        write!(s, "{k}: {v}\r\n").unwrap();
    }
    write!(s, "\r\n{body}").unwrap();
    let mut response = String::new();
    s.take(65536).read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut cookie = None;
    let mut access = None;
    for (k, v) in head.lines().filter_map(|h| h.split_once(':')) {
        assert!(
            !k.eq_ignore_ascii_case("Location")
                && !k.to_ascii_lowercase().starts_with("access-control-allow-")
        );
        if k.eq_ignore_ascii_case("Set-Cookie") {
            assert!(cookie.replace(v.trim().to_owned()).is_none());
        }
        if k.eq_ignore_ascii_case("boardcraft-board-access") {
            access = Some(v.trim().to_owned());
        }
    }
    Reply {
        status,
        access,
        cookie,
        body: body.to_owned(),
    }
}

#[test]
fn disposable_viewer_opens_review_on_independent_visits() {
    use boardcraft_mobile_edit_client::{DISPOSABLE_VIEWER, VIEWER_BOARD_PATH};
    use boardcraft_mobile_edit_host::REVIEW_OWNER;
    use uuid::Uuid;
    let f = Fixture::start();
    let board = BoardId::new(f.board).unwrap();
    let dist = tempfile::tempdir().unwrap();
    for name in ["index.html", "shell.wasm", "shell.js"] {
        std::fs::write(dist.path().join(name), []).unwrap();
    }
    let entry = format!("/web/api{VIEWER_BOARD_PATH}");
    let path = format!("/web/api/navigation/boards/{}", f.board);
    let get = |host: &Host, path: &str| {
        let authority = format!("127.0.0.1:{}", host.port);
        send(
            host.port,
            "GET",
            path,
            &authority,
            &[("Boardcraft-Web", DISPOSABLE_VIEWER)],
            "",
        )
    };
    for normal in [
        Host::start_web_api(&f.schema.config, board).unwrap(),
        Host::start_web(&f.schema.config, board, dist.path()).unwrap(),
    ] {
        assert_eq!(get(&normal, &entry).status, 404);
        assert_eq!(get(&normal, &path).status, 401);
    }

    let host = Host::start_disposable_viewer(&f.schema.config, board, dist.path()).unwrap();
    let mut db = f.connect();
    let workspace = Uuid::new_v4();
    db.execute("INSERT INTO workspaces(id,creator_id,name,position,active,is_open) VALUES ($1,$2,'Review',0,true,false)", &[&workspace,&REVIEW_OWNER]).unwrap();
    db.execute("INSERT INTO board_policies(board_id,workspace_id,name,position,kind,creator_id,active) VALUES ($1,$2,'Review',0,'private',$3,true)", &[&f.board,&workspace,&REVIEW_OWNER]).unwrap();
    let item = Uuid::new_v4();
    let authority = format!("127.0.0.1:{}", host.port);
    let origin = format!("http://{authority}");
    for visit in 0..2 {
        let landing = get(&host, &entry);
        assert_eq!(landing.status, 200);
        assert_eq!(
            serde_json::from_str::<Uuid>(&landing.body).unwrap(),
            f.board
        );
        assert!(landing.cookie.is_none());
        let reply = get(&host, &path);
        assert_eq!((reply.status, reply.access.as_deref()), (200, Some("edit")));
        assert!(reply.cookie.is_none());
        if visit == 0 {
            let body =
                serde_json::json!({"id":item,"group_id":"main","title":"Sketch"}).to_string();
            let create = |origin| {
                send(
                    host.port,
                    "POST",
                    &format!("{path}/items"),
                    &authority,
                    &[("Origin", origin), ("Boardcraft-Web", DISPOSABLE_VIEWER)],
                    &body,
                )
                .status
            };
            assert_eq!(create("http://localhost"), 403);
            assert_eq!(create(&origin), 200);
        } else {
            let groups: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
            assert!(
                groups[0]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == item.to_string())
            );
        }
    }
    let creator: Uuid = db
        .query_one("SELECT created_by FROM board_items WHERE id=$1", &[&item])
        .unwrap()
        .get(0);
    assert_eq!(creator, REVIEW_OWNER);
    assert_no_session(&f);
}

pub(super) fn groups() {
    use selection::SELECTED;
    use serde_json::{Value, json};
    use uuid::Uuid;
    let f = Fixture::start();
    let password = Uuid::new_v4().to_string();
    let member = f.seed_account("reader", &password, Role::Member);
    let catalog = selection::Catalog::new(&f, member);
    selection::seed_groups(&f);
    let host = Host::start_web_api(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
    let authority = format!("127.0.0.1:{}", host.port);
    let origin = format!("http://{authority}");
    let call = |method: &str, path: &str, fields: &[(&str, &str)], body: &str| {
        send(host.port, method, path, &authority, fields, body)
    };
    let login = call(
        "POST",
        "/web/api/session",
        &[("Origin", &origin), ("Boardcraft-Web", "login")],
        &json!({"identifier":"reader","password":password}).to_string(),
    );
    assert_eq!(login.status, 200);
    let binding: String = serde_json::from_str(&login.body).unwrap();
    let cookie = login.cookie.as_deref().unwrap().split(';').next().unwrap();
    let pair = [("Cookie", cookie), ("Boardcraft-Web", binding.as_str())];
    let session = StoredSession::only(&f, member);
    let path = format!("/web/api/navigation/boards/{SELECTED}");
    for half in [&pair[0..1], &pair[1..2]] {
        let reply = call("GET", &path, half, "");
        assert!(matches!(reply.status, 401 | 403) && reply.body.is_empty());
    }
    let row = |n, title| json!({"id":Uuid::from_u128(n),"title":title,"revision":1});
    let expected = json!([
        {"id":"g-90","name":"Work","colour":"#579bfc","revision":1,"collapsed":false,"items":[row(73,"Sketch"),row(61,"Ink")]},
        {"id":"g-10","name":"Reserve","colour":"#579bfc","revision":1,"collapsed":false,"items":[]},
        {"id":"g-20","name":"Work","colour":"#579bfc","revision":1,"collapsed":false,"items":[row(59,"Ink")]}
    ]);
    for (role, hint) in [("viewer", "read"), ("member", "edit")] {
        f.connect()
            .execute("UPDATE accounts SET role=$2 WHERE id=$1", &[&member, &role])
            .unwrap();
        let reply = call("GET", &path, &pair, "");
        assert_eq!((reply.status, reply.access.as_deref()), (200, Some(hint)));
        assert_eq!(
            serde_json::from_str::<Value>(&reply.body).unwrap(),
            expected
        );
    }
    let deny_put = |id| {
        let reply = call(
            "PUT",
            &format!("{path}/items/{}/name", Uuid::from_u128(id)),
            &[pair[0], pair[1], ("Origin", &origin)],
            r#"{"revision":1,"title":"Coral"}"#,
        );
        assert!(reply.status == 404 && reply.body.is_empty());
    };
    deny_put(57);
    let active = |board: Uuid, group: &str, value: bool| {
        assert_eq!(
            f.connect()
                .execute(
                    "UPDATE board_groups SET active=$3 WHERE board_id=$1 AND id=$2",
                    &[&board, &group, &value]
                )
                .unwrap(),
            1
        )
    };
    active(SELECTED, "g-20", false);
    deny_put(59);
    active(SELECTED, "g-20", true);
    catalog.membership(&f, false);
    assert_eq!(call("GET", &path, &pair, "").status, 403);
    catalog.membership(&f, true);
    assert!(StoredSession::only(&f, member) == session);
    let native = boardcraft_account::sign_in(&f.schema.config, "reader", &password).unwrap();
    let client = EditClient::with_bearer(host.port, native.encoded()).unwrap();
    assert_eq!(
        serde_json::to_value(client.read_selected_board(SELECTED).unwrap()).unwrap(),
        json!([row(73, "Sketch"), row(61, "Ink"), row(59, "Ink")])
    );
    active(f.board, "main", false);
    assert!(client.read(f.board).unwrap().is_empty());
    assert_eq!(
        client.rename(
            f.board,
            super::fixture::SECOND,
            &boardcraft_mobile_edit_client::NameEdit {
                revision: 1,
                title: "Coral".into()
            }
        ),
        Err(boardcraft_mobile_edit_client::EditError::Missing)
    );
    active(f.board, "main", true);
    catalog.check_grants(&f, true);
    selection::check_groups(&f, false);
}

pub(super) fn check() {
    let f = Fixture::start();
    let board = BoardId::new(f.board).unwrap();
    let password = uuid::Uuid::new_v4().to_string();
    let member = f.seed_account("reader", &password, Role::Member);
    let catalog = selection::Catalog::new(&f, member);
    let host = Host::start_web_api(&f.schema.config, board).unwrap();
    let nav = Host::start_with_navigation(&f.schema.config, board).unwrap();
    let authority = format!("127.0.0.1:{}", host.port);
    let origin = format!("http://{authority}");
    let foreign = format!("http://127.0.0.1:{}", nav.port);
    let login = serde_json::json!({"identifier":"reader","password":password}).to_string();
    let api = "/web/api/session";
    let binding = "Boardcraft-Web";
    let exact = ("Origin", origin.as_str());
    let start = (binding, "login");
    let local = |method, path, fields: &[(&str, &str)], body| {
        send(host.port, method, path, &authority, fields, body)
    };
    fn denied(reply: Reply) {
        assert!(
            (400..500).contains(&reply.status) && reply.cookie.is_none() && reply.body.is_empty()
        );
    }
    for fields in [
        vec![start],
        vec![exact],
        vec![start, ("Origin", "null")],
        vec![start, ("Origin", foreign.as_str())],
        vec![start, exact, exact],
        vec![start, exact, ("Authorization", "Bearer invalid")],
    ] {
        denied(local("POST", api, &fields, &login));
    }
    denied(send(
        host.port,
        "POST",
        api,
        "localhost",
        &[start, exact],
        &login,
    ));
    denied(local("POST", "/session", &[exact], &login));
    assert_no_session(&f);
    let reply = local("POST", api, &[exact, start], &login);
    assert_eq!(reply.status, 200);
    let issued = reply.cookie.unwrap();
    let mut parts = issued.split(';').map(str::trim);
    let cookie = parts.next().unwrap();
    let flags = parts.collect::<Vec<_>>();
    assert!(
        flags.len() == 3
            && flags.contains(&"HttpOnly")
            && flags.contains(&"SameSite=Strict")
            && flags.contains(&"Path=/web/api")
    );
    let key: String = serde_json::from_str(&reply.body).unwrap();
    let handle = cookie.split_once('=').unwrap().1;
    assert!(
        handle != key
            && [key.as_str(), handle]
                .iter()
                .all(|v| Token::from_encoded(v).is_err())
    );
    let pair = [("Cookie", cookie), (binding, key.as_str())];
    let first = StoredSession::only(&f, member);
    for fields in [&pair[..1], &pair[1..]] {
        denied(local("GET", api, fields, ""));
    }
    denied(local("GET", api, &[pair[0], pair[1], pair[1]], ""));
    denied(local(
        "GET",
        api,
        &[pair[0], pair[1], ("Origin", &foreign)],
        "",
    ));
    denied(local("GET", "/session", &pair, ""));
    for port in [f.host.port, nav.port] {
        denied(send(
            port,
            "GET",
            api,
            &format!("127.0.0.1:{port}"),
            &pair,
            "",
        ));
    }
    assert_eq!(local("GET", api, &pair, "").status, 200);
    // Cookies share the loopback host, not its port. Even both halves from this
    // origin must not select a session in another web host's independent table.
    let peer = Host::start_web_api(&f.schema.config, board).unwrap();
    denied(send(
        peer.port,
        "GET",
        api,
        &format!("127.0.0.1:{}", peer.port),
        &pair,
        "",
    ));
    assert_eq!(local("GET", api, &pair, "").status, 200);
    let selected = format!("/web/api/navigation/boards/{}", selection::SELECTED);
    assert_eq!(local("GET", &selected, &pair, "").status, 200);
    let edit = format!("{selected}/items/{}/name", uuid::Uuid::from_u128(61));
    for (method, path, body) in [
        ("PUT", edit.as_str(), r#"{"title":"Coral","revision":1}"#),
        ("POST", "/web/api/session/logout", ""),
    ] {
        for extra in [
            vec![],
            vec![("Origin", "null")],
            vec![("Origin", foreign.as_str())],
            vec![exact, exact],
            vec![exact, ("Authorization", "Bearer invalid")],
        ] {
            let mut fields = pair.to_vec();
            fields.extend(extra);
            denied(local(method, path, &fields, body));
        }
        denied(send(
            host.port,
            method,
            path,
            "localhost",
            &[pair[0], pair[1], exact],
            body,
        ));
    }
    assert!(first == StoredSession::only(&f, member));
    catalog.check(&f, true);
    let native = boardcraft_account::sign_in(&f.schema.config, "reader", &password).unwrap();
    let client = EditClient::with_bearer(host.port, native.encoded()).unwrap();
    client.resolve().unwrap();
    let mixed = ("Authorization", format!("Bearer {}", native.encoded()));
    denied(local(
        "GET",
        api,
        &[pair[0], pair[1], (mixed.0, &mixed.1)],
        "",
    ));
    denied(local(
        "GET",
        "/session",
        &[pair[0], pair[1], (mixed.0, &mixed.1)],
        "",
    ));
    assert_eq!(
        local(
            "POST",
            "/web/api/session/logout",
            &[pair[0], pair[1], exact],
            ""
        )
        .status,
        200
    );
    denied(local("GET", api, &pair, ""));
    client.resolve().unwrap();
    assert!(first != StoredSession::only(&f, member));
    catalog.check(&f, true);
}
