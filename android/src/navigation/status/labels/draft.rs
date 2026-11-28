//! The Status editor's exact account/origin/column draft; never session material.
use boardcraft_mobile_edit_client::StatusLabelEdit;
use gpui_mobile::android::jni::{activity, get_string, with_env};
use jni::{jni_sig, jni_str};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
use uuid::Uuid;

type Patches = Vec<(String, StatusLabelEdit)>;

pub(super) struct Journal {
    file: Option<PathBuf>,
    bytes: Option<Vec<u8>>,
    pub(super) unreadable: bool,
    pub(super) storage_error: bool,
}

impl Journal {
    pub(super) fn open(port: u16, actor: Uuid, board: Uuid, column: Uuid) -> (Self, Patches) {
        let file = with_env(|env| {
            let app = activity(env)?;
            let directory = env
                .call_method(
                    &app,
                    jni_str!("getNoBackupFilesDir"),
                    jni_sig!("()Ljava/io/File;"),
                    &[],
                )
                .and_then(|value| value.l())
                .map_err(|e| e.to_string())?;
            let name = env
                .call_method(
                    &directory,
                    jni_str!("getAbsolutePath"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )
                .and_then(|value| value.l())
                .map_err(|e| e.to_string())?;
            let directory = PathBuf::from(get_string(env, &name));
            Ok(directory.join(format!(
                "status-labels-{port}-{actor}-{board}-{column}.json"
            )))
        })
        .ok();
        let mut journal = Self {
            file,
            bytes: None,
            unreadable: false,
            storage_error: false,
        };
        let patches = journal.recover().unwrap_or_default();
        (journal, patches)
    }

    pub(super) fn recover(&mut self) -> Option<Patches> {
        let Some(file) = &self.file else {
            self.unreadable = true;
            return None;
        };
        let input = match fs::File::open(file) {
            Ok(input) => input,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.bytes = None;
                self.unreadable = false;
                return Some(Vec::new());
            }
            Err(_) => {
                self.unreadable = true;
                return None;
            }
        };
        let mut bytes = Vec::new();
        if input.take(400_001).read_to_end(&mut bytes).is_err() {
            self.unreadable = true;
            return None;
        }
        let patches = serde_json::from_slice::<Patches>(&bytes)
            .ok()
            .filter(|patches| {
                bytes.len() <= 400_000
                    && patches.len() <= 250
                    && patches.iter().enumerate().all(|(ix, (key, edit))| {
                        !key.is_empty()
                            && key.len() <= 128
                            && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                            && patches[..ix].iter().all(|(id, _)| id != key)
                            && (edit.label.is_some() || edit.color.is_some())
                            && edit
                                .label
                                .as_ref()
                                .is_none_or(|name| name.encode_utf16().count() <= 160)
                            && edit.color.as_ref().is_none_or(|color| {
                                color.len() == 7
                                    && color.starts_with('#')
                                    && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
                            })
                    })
            });
        if self.bytes.is_none() || patches.is_some() {
            self.bytes = Some(bytes);
        }
        self.unreadable = patches.is_none();
        patches
    }

    pub(super) fn retain(&mut self, patches: Patches) -> bool {
        if self.unreadable {
            return false;
        }
        let Some(file) = &self.file else {
            self.storage_error = true;
            return false;
        };
        let bytes = if patches.is_empty() {
            None
        } else {
            serde_json::to_vec(&patches).ok()
        };
        if self.bytes == bytes {
            return true;
        }
        let result = if let Some(bytes) = &bytes {
            let pending = file.with_extension("pending");
            (|| -> std::io::Result<()> {
                let mut options = fs::OpenOptions::new();
                options.create(true).write(true).truncate(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut output = options.open(&pending)?;
                output.write_all(bytes)?;
                output.sync_all()?;
                drop(output);
                fs::rename(pending, file)
            })()
        } else {
            match fs::remove_file(file) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        };
        self.storage_error = result.is_err();
        if result.is_ok() {
            self.bytes = bytes;
        }
        result.is_ok()
    }

    pub(super) fn discard(&mut self) -> bool {
        let Some(file) = &self.file else {
            return false;
        };
        let Ok(current) = fs::read(file) else {
            return false;
        };
        if self.bytes.as_deref() != Some(current.as_slice()) {
            return false;
        }
        if fs::remove_file(file).is_err() {
            return false;
        }
        self.bytes = None;
        self.unreadable = false;
        self.storage_error = false;
        true
    }
}
