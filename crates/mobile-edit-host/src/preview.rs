use std::io::Cursor;
use tiny_http::{Header, Method, Request, Response};

pub(super) fn asset(request: &Request) -> Option<Response<Cursor<&'static [u8]>>> {
    if request.method() != &Method::Get {
        return None;
    }
    let (mime, bytes): (&str, &[u8]) = match request.url() {
        "/preview/" => (
            "text/html; charset=utf-8",
            include_bytes!("preview/index.html"),
        ),
        "/preview/page.js" => ("text/javascript", include_bytes!("preview/page.js")),
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
