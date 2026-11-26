use std::io::Cursor;
use tiny_http::{Header, Method, Request, Response};
use uuid::Uuid;

fn board_document(request: &Request) -> bool {
    let Some(text) = request.url().strip_prefix("/boards/") else {
        return false;
    };
    let has = |name, value: &str| {
        request
            .headers()
            .iter()
            .any(|h| h.field.equiv(name) && h.value.as_str() == value)
    };
    Uuid::parse_str(text).is_ok_and(|id| !id.is_nil() && id.to_string() == text)
        && has("Sec-Fetch-Mode", "navigate")
        && has("Sec-Fetch-Dest", "document")
}

pub(super) fn asset(request: &Request) -> Option<Response<Cursor<&'static [u8]>>> {
    if request.method() != &Method::Get {
        return None;
    }
    let (mime, bytes): (&str, &[u8]) = match request.url() {
        path if path == "/preview/" || board_document(request) => (
            "text/html; charset=utf-8",
            include_bytes!("preview/index.html"),
        ),
        "/preview/page.js" => ("text/javascript", include_bytes!("preview/page.js")),
        "/preview/names.js" => ("text/javascript", include_bytes!("preview/names.js")),
        "/preview/page.css" => ("text/css", include_bytes!("preview/page.css")),
        _ => return None,
    };
    let mut response = Response::new(
        200.into(),
        vec![],
        Cursor::new(bytes),
        Some(bytes.len()),
        None,
    );
    for (key, value) in [
        ("Content-Type", mime),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        (
            "Content-Security-Policy",
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    ] {
        response.add_header(Header::from_bytes(key, value).unwrap());
    }
    Some(response)
}
