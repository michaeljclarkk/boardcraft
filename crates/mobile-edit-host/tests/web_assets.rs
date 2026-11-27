//! Host routing and asset isolation only; this is not a browser or UI test.
#[allow(dead_code)]
mod fixture;

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpStream},
    time::Duration,
};

use boardcraft_board::BoardId;
use boardcraft_mobile_edit_host::Host;
use uuid::Uuid;

fn get(port: u16, path: &str, authority: &str) -> (u16, String, Vec<u8>) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(stream, "GET {path} HTTP/1.0\r\nHost: {authority}\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let boundary = bytes.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
    let headers = String::from_utf8(bytes[..boundary].to_vec())
        .unwrap()
        .to_ascii_lowercase();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, headers, bytes[boundary + 4..].to_vec())
}

#[test]
fn web_assets_preserve_isolation_and_the_login_boundary() {
    // Deliberately tiny synthetic assets test the host, not wasm rendering.
    let dist = tempfile::tempdir().unwrap();
    let page = b"<!doctype html><title>Review</title>";
    let wasm = b"\0asm\x01\0\0\0";
    for (name, bytes) in [
        ("index.html", page.as_slice()),
        ("shell.js", b"".as_slice()),
        ("shell_bg.wasm", wasm.as_slice()),
    ] {
        std::fs::write(dist.path().join(name), bytes).unwrap();
    }
    let schema = fixture::Schema::new();
    let board = Uuid::new_v4();
    let host = Host::start_web(&schema.config, BoardId::new(board).unwrap(), dist.path()).unwrap();
    let authority = format!("127.0.0.1:{}", host.port);
    for (path, mime, expected) in [
        (
            "/web/".to_owned(),
            "text/html; charset=utf-8",
            page.as_slice(),
        ),
        (
            format!("/web/boards/{board}"),
            "text/html; charset=utf-8",
            page.as_slice(),
        ),
        (
            "/web/shell.js".to_owned(),
            "text/javascript; charset=utf-8",
            b"".as_slice(),
        ),
        (
            "/web/shell_bg.wasm".to_owned(),
            "application/wasm",
            wasm.as_slice(),
        ),
    ] {
        let (status, headers, body) = get(host.port, &path, &authority);
        assert_eq!(status, 200);
        assert_eq!(body, expected);
        for header in [
            format!("content-type: {mime}"),
            "cache-control: no-store".into(),
            "x-content-type-options: nosniff".into(),
            "cross-origin-opener-policy: same-origin".into(),
            "cross-origin-embedder-policy: require-corp".into(),
            "cross-origin-resource-policy: same-origin".into(),
            "content-security-policy: default-src 'none'".into(),
        ] {
            assert!(headers.contains(&header));
        }
        assert!(!headers.contains("set-cookie:") && !headers.contains("access-control-allow-"));
    }
    for path in [
        "/preview/",
        "/preview/page.js",
        "/preview/page.css",
        "/preview/api/session",
        "/web/apiary/session",
        "/web/src/lib.rs",
        "/web/../Cargo.toml",
        "/web/boards/not-a-board",
        "/web/boards/00000000-0000-0000-0000-000000000000",
    ] {
        let (status, _, body) = get(host.port, path, &authority);
        assert_eq!(status, 404);
        assert!(body.is_empty());
    }
    for path in [
        "/web/api/session".to_owned(),
        format!("/web/api/navigation/boards/{board}"),
    ] {
        let (status, _, body) = get(host.port, &path, &authority);
        assert_eq!(status, 401);
        assert!(body.is_empty());
    }
    let (status, _, body) = get(host.port, "/web/", "localhost");
    assert_eq!(status, 403);
    assert!(body.is_empty());
}
