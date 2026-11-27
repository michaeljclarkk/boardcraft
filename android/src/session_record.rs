//! Android's versioned, exact-origin encrypted record format; host tests exercise disk recovery.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) const VERSION: u8 = 1;
pub(crate) const IV_BYTES: usize = 12;
pub(crate) const TOKEN_BYTES: usize = 43;
pub(crate) const TAG_BYTES: usize = 16;
const RECORD_BYTES: usize = 1 + IV_BYTES + TOKEN_BYTES + TAG_BYTES;

pub(crate) fn valid_token(token: &[u8]) -> bool {
    token.len() == TOKEN_BYTES
        && token
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

pub(crate) fn aad(port: u16) -> io::Result<Vec<u8>> {
    if port == 0 {
        return Err(io::Error::other("invalid session origin"));
    }
    Ok(format!("{VERSION} http://127.0.0.1:{port}").into_bytes())
}

pub(crate) fn path(directory: &Path, port: u16) -> io::Result<PathBuf> {
    if port == 0 || !directory.is_dir() {
        return Err(io::Error::other("invalid session directory or origin"));
    }
    Ok(directory.join(format!("http-127.0.0.1-{port}")))
}

fn pending(file: &Path) -> io::Result<PathBuf> {
    let name = file
        .file_name()
        .ok_or_else(|| io::Error::other("invalid session path"))?;
    let mut name = name.to_os_string();
    name.push(".pending");
    Ok(file.with_file_name(name))
}

pub(crate) fn load(file: &Path) -> io::Result<Option<(Vec<u8>, Vec<u8>)>> {
    let input = match File::open(file) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    input
        .take(RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != RECORD_BYTES || bytes.first() != Some(&VERSION) {
        return Err(io::Error::other("unreadable session format"));
    }
    let iv = bytes
        .get(1..1 + IV_BYTES)
        .ok_or_else(|| io::Error::other("invalid IV"))?
        .to_vec();
    let sealed = bytes
        .get(1 + IV_BYTES..)
        .ok_or_else(|| io::Error::other("invalid sealed session"))?
        .to_vec();
    Ok(Some((iv, sealed)))
}

pub(crate) fn save(file: &Path, iv: &[u8], sealed: &[u8]) -> io::Result<()> {
    if iv.len() != IV_BYTES || sealed.len() != TOKEN_BYTES + TAG_BYTES {
        return Err(io::Error::other("unexpected GCM parameters"));
    }
    let tmp = pending(file)?;
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&tmp)?;
        output.write_all(&[VERSION])?;
        output.write_all(iv)?;
        output.write_all(sealed)?;
        output.sync_all()?;
        drop(output);
        fs::rename(&tmp, file)?; // Android/Linux atomic replace on the same filesystem.
        Ok(())
    })();
    let cleanup = remove_if_exists(&tmp);
    result.and(cleanup)
}

fn remove_if_exists(file: &Path) -> io::Result<()> {
    match fs::remove_file(file) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub(crate) fn forget(file: &Path) -> io::Result<()> {
    remove_if_exists(file)?;
    remove_if_exists(&pending(file)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_roundtrip_is_exact_origin_bounded_and_keeps_bad_data_for_retry() {
        let work = tempfile::tempdir().unwrap();
        let a = path(work.path(), 9001).unwrap();
        let b = path(work.path(), 9002).unwrap();
        assert!(load(&a).unwrap().is_none());
        let iv = vec![7; IV_BYTES];
        let sealed = vec![9; TOKEN_BYTES + TAG_BYTES];
        save(&a, &iv, &sealed).unwrap();
        assert_eq!(load(&a).unwrap(), Some((iv, sealed)));
        assert!(load(&b).unwrap().is_none());
        assert_ne!(aad(9001).unwrap(), aad(9002).unwrap());
        assert!(path(work.path(), 0).is_err());
        assert!(aad(0).is_err());
        for bytes in [
            vec![],
            vec![VERSION; RECORD_BYTES - 1],
            vec![VERSION; RECORD_BYTES + 1],
            vec![2; RECORD_BYTES],
        ] {
            fs::write(&a, &bytes).unwrap();
            assert!(load(&a).is_err());
            assert_eq!(fs::read(&a).unwrap(), bytes);
        }
        fs::write(pending(&a).unwrap(), b"interrupted encrypted record").unwrap();
        forget(&a).unwrap();
        forget(&a).unwrap();
        assert!(!a.exists());
        assert!(!pending(&a).unwrap().exists());
    }

    #[test]
    fn compatible_record_replaces_atomically_and_forgets_only_selected_origin() {
        let work = tempfile::tempdir().unwrap();
        let a = path(work.path(), 9001).unwrap();
        let b = path(work.path(), 9002).unwrap();
        assert_eq!(a.file_name().unwrap(), "http-127.0.0.1-9001");
        assert_eq!(aad(9001).unwrap(), b"1 http://127.0.0.1:9001");
        let iv = [7; IV_BYTES];
        let sealed = [9; TOKEN_BYTES + TAG_BYTES];
        save(&a, &iv, &sealed).unwrap();
        assert_eq!(
            fs::read(&a).unwrap(),
            [vec![1], iv.to_vec(), sealed.to_vec()].concat()
        );
        save(&a, &[8; IV_BYTES], &sealed).unwrap();
        assert_eq!(
            load(&a).unwrap(),
            Some((vec![8; IV_BYTES], sealed.to_vec()))
        );
        assert!(!pending(&a).unwrap().exists());
        save(&b, &iv, &sealed).unwrap();
        fs::write(pending(&a).unwrap(), b"interrupted encrypted write").unwrap();
        forget(&a).unwrap();
        assert!(!a.exists());
        assert!(!pending(&a).unwrap().exists());
        assert_eq!(load(&b).unwrap(), Some((iv.to_vec(), sealed.to_vec())));
    }
    #[test]
    fn invalid_cipher_output_never_replaces_accepted_record() {
        let work = tempfile::tempdir().unwrap();
        let file = path(work.path(), 9001).unwrap();
        fs::write(&file, b"prior").unwrap();
        assert!(save(&file, &[0; 11], &[0; 59]).is_err());
        assert!(save(&file, &[0; IV_BYTES], &[0; 58]).is_err());
        assert_eq!(fs::read(file).unwrap(), b"prior");
        assert!(valid_token(&[b'a'; 43]));
        assert!(!valid_token(&[b'a'; 42]));
        assert!(!valid_token(&[b' '; 43]));
    }
}
