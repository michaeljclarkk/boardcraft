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
struct Reply {
    status: u16,
    cookie: Option<String>,
    body: String,
}
fn send(
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
    for (k, v) in head.lines().filter_map(|h| h.split_once(':')) {
        assert!(
            !k.eq_ignore_ascii_case("Location")
                && !k.to_ascii_lowercase().starts_with("access-control-allow-")
        );
        if k.eq_ignore_ascii_case("Set-Cookie") {
            assert!(cookie.replace(v.trim().to_owned()).is_none());
        }
    }
    Reply {
        status,
        cookie,
        body: body.to_owned(),
    }
}

pub(super) fn check() {
    let f = Fixture::start();
    let board = BoardId::new(f.board).unwrap();
    let password = uuid::Uuid::new_v4().to_string();
    let member = f.seed_account("reader", &password, Role::Member);
    let catalog = selection::Catalog::new(&f, member);
    let host = Host::start_preview(&f.schema.config, board).unwrap();
    let nav = Host::start_with_navigation(&f.schema.config, board).unwrap();
    let authority = format!("127.0.0.1:{}", host.port);
    let origin = format!("http://{authority}");
    let foreign = format!("http://127.0.0.1:{}", nav.port);
    let login = serde_json::json!({"identifier":"reader","password":password}).to_string();
    let api = "/preview/api/session";
    let binding = "Boardcraft-Preview";
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
            && flags.contains(&"Path=/preview/api")
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
    let selected = format!("/preview/api/navigation/boards/{}", selection::SELECTED);
    assert_eq!(local("GET", &selected, &pair, "").status, 200);
    let edit = format!("{selected}/items/{}/name", uuid::Uuid::from_u128(61));
    for (method, path, body) in [
        ("PUT", edit.as_str(), r#"{"title":"Coral","revision":1}"#),
        ("POST", "/preview/api/session/logout", ""),
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
            "/preview/api/session/logout",
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
