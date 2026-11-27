//! A bounded snapshot of an explicitly supplied Trunk output directory. No
//! arbitrary paths, symlinks, source files or credentials can be requested.
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

use tiny_http::{Header, Method, Request, Response};

const MAX_BUNDLE: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 256;

pub(super) struct Assets(HashMap<String, (String, Arc<[u8]>)>);

impl Assets {
    pub(super) fn load(dist: &Path) -> Result<Self, &'static str> {
        let root = dist.canonicalize().map_err(|_| "web bundle unavailable")?;
        let mut assets = Self(HashMap::new());
        let mut remaining = MAX_BUNDLE;
        assets.read_directory(&root, &root, &mut remaining)?;
        if !assets.0.contains_key("/web/index.html")
            || !assets.0.keys().any(|key| key.ends_with(".wasm"))
            || !assets.0.keys().any(|key| key.ends_with(".js"))
        {
            return Err("expected a Trunk-built web bundle");
        }
        Ok(assets)
    }

    fn read_directory(
        &mut self,
        root: &Path,
        directory: &Path,
        remaining: &mut u64,
    ) -> Result<(), &'static str> {
        for entry in std::fs::read_dir(directory).map_err(|_| "web bundle unreadable")? {
            let entry = entry.map_err(|_| "web bundle unreadable")?;
            let kind = entry.file_type().map_err(|_| "web bundle unreadable")?;
            if kind.is_symlink() {
                return Err("web bundle must not contain symlinks");
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "invalid web bundle path")?;
            if relative.components().count() > 8 {
                return Err("web bundle directory too deep");
            }
            if relative == Path::new(".trunk-lock") {
                continue;
            }
            if kind.is_dir() {
                self.read_directory(root, &path, remaining)?;
                continue;
            }
            let mime = match path.extension().and_then(|value| value.to_str()) {
                Some("html") if relative == Path::new("index.html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript; charset=utf-8",
                Some("wasm") => "application/wasm",
                Some("txt") if relative == Path::new("LICENSE.txt") => "text/plain; charset=utf-8",
                _ => return Err("unexpected web bundle file"),
            };
            if !kind.is_file() || self.0.len() >= MAX_FILES {
                return Err("web bundle file limit exceeded");
            }
            let length = entry.metadata().map_err(|_| "web bundle unreadable")?.len();
            *remaining = remaining
                .checked_sub(length)
                .ok_or("web bundle size limit exceeded")?;
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .map_err(|_| "web bundle unreadable")?
                .take(length + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "web bundle unreadable")?;
            if bytes.len() as u64 != length {
                return Err("web bundle changed during load");
            }
            let key = format!("/web/{}", relative.to_string_lossy().replace('\\', "/"));
            self.0.insert(key, (mime.into(), bytes.into()));
        }
        Ok(())
    }

    pub(super) fn response(&self, request: &Request) -> Option<Response<Cursor<Arc<[u8]>>>> {
        if request.method() != &Method::Get {
            return None;
        }
        let path = request.url().split('?').next()?;
        let board_address = path.strip_prefix("/web/boards/").is_some_and(|text| {
            uuid::Uuid::parse_str(text).is_ok_and(|id| !id.is_nil() && id.to_string() == text)
        });
        let path = if path == "/web/" || board_address {
            "/web/index.html"
        } else {
            path
        };
        let (mime, bytes) = self.0.get(path)?;
        let mut response = Response::new(
            200.into(),
            vec![],
            Cursor::new(bytes.clone()),
            Some(bytes.len()),
            None,
        );
        for (name, value) in [
            ("Content-Type", mime.as_str()),
            ("Cache-Control", "no-store"),
            ("X-Content-Type-Options", "nosniff"),
            ("Cross-Origin-Opener-Policy", "same-origin"),
            ("Cross-Origin-Embedder-Policy", "require-corp"),
            ("Cross-Origin-Resource-Policy", "same-origin"),
            // Inline script is Trunk's generated module bootstrap only. No
            // application logic or user-provided content is in this page.
            (
                "Content-Security-Policy",
                "default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' blob:; worker-src 'self' blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
        ] {
            response.add_header(Header::from_bytes(name, value).expect("static response header"));
        }
        Some(response)
    }
}

#[cfg(test)]
mod tests {
    use super::Assets;

    #[test]
    fn bundle_snapshot_cannot_serve_later_filesystem_edits() {
        let directory = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("index.html", "Review"),
            ("shell.js", ""),
            ("shell.wasm", ""),
            (".trunk-lock", ""),
        ] {
            std::fs::write(directory.path().join(name), bytes).unwrap();
        }
        let assets = Assets::load(directory.path()).unwrap();
        std::fs::write(directory.path().join("index.html"), "Ink").unwrap();
        std::fs::write(directory.path().join("credentials.json"), "Quartz").unwrap();
        assert_eq!(assets.0["/web/index.html"].1.as_ref(), b"Review");
        assert!(!assets.0.contains_key("/web/credentials.json"));
        assert!(Assets::load(directory.path()).is_err());
    }
}
