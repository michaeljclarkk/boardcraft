//! Android Keystore AES-GCM through checked JNI calls. No app-owned Java helper.
//! Call on a worker with an attached environment and the current Activity/Context.
use crate::session_record;
use jni::{
    Env, jni_sig, jni_str,
    objects::{JByteArray, JObject, JString, JValue},
};
use std::{path::PathBuf, sync::Mutex};

static ACCESS: Mutex<()> = Mutex::new(());
const ALIAS: &str = "boardcraft.session.aes";

/// A local storage fault must stay distinct from a missing record or remote rejection.
#[derive(Debug)]
pub(crate) enum StorageError {
    /// The origin's record or no-backup directory could not be accessed.
    Io(std::io::Error),
    /// An Android platform call failed; the application boundary clears exceptions.
    Jni(jni::errors::Error),
    /// The key or authenticated token is invalid. Preserve the record for recovery.
    Invalid,
    /// A prior storage operation failed while holding the serialized boundary.
    Lock,
}
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<jni::errors::Error> for StorageError {
    fn from(e: jni::errors::Error) -> Self {
        Self::Jni(e)
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Io(_) => "session record I/O failed",
            Self::Jni(_) => "session platform call failed",
            Self::Invalid => "invalid session key or token",
            Self::Lock => "session storage lock unavailable",
        })
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Jni(error) => Some(error),
            Self::Invalid | Self::Lock => None,
        }
    }
}

fn record(env: &mut Env<'_>, context: &JObject<'_>, port: u16) -> Result<PathBuf, StorageError> {
    let directory = env
        .call_method(
            context,
            jni_str!("getNoBackupFilesDir"),
            jni_sig!("()Ljava/io/File;"),
            &[],
        )?
        .l()?;
    let name = env
        .call_method(
            &directory,
            jni_str!("getAbsolutePath"),
            jni_sig!("()Ljava/lang/String;"),
            &[],
        )?
        .l()?;
    let name = env.cast_local::<JString>(name)?.to_string();
    Ok(session_record::path(&PathBuf::from(name), port)?)
}

fn key<'local>(env: &mut Env<'local>, create: bool) -> Result<JObject<'local>, StorageError> {
    let provider = env.new_string("AndroidKeyStore")?;
    let store = env
        .call_static_method(
            jni_str!("java/security/KeyStore"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;)Ljava/security/KeyStore;"),
            &[JValue::Object(&provider)],
        )?
        .l()?;
    env.call_method(
        &store,
        jni_str!("load"),
        jni_sig!("(Ljava/security/KeyStore$LoadStoreParameter;)V"),
        &[JValue::Object(&JObject::null())],
    )?;
    let alias = env.new_string(ALIAS)?;
    let key = env
        .call_method(
            &store,
            jni_str!("getKey"),
            jni_sig!("(Ljava/lang/String;[C)Ljava/security/Key;"),
            &[JValue::Object(&alias), JValue::Object(&JObject::null())],
        )?
        .l()?;
    if !key.is_null() {
        return Ok(key);
    }
    if !create {
        return Err(StorageError::Invalid);
    }
    let algorithm = env.new_string("AES")?;
    let generator = env
        .call_static_method(
            jni_str!("javax/crypto/KeyGenerator"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;"),
            &[JValue::Object(&algorithm), JValue::Object(&provider)],
        )?
        .l()?;
    let builder = env.new_object(
        jni_str!("android/security/keystore/KeyGenParameterSpec$Builder"),
        jni_sig!("(Ljava/lang/String;I)V"),
        &[JValue::Object(&alias), JValue::Int(3)],
    )?;
    let gcm = env.new_string("GCM")?;
    let modes = env.new_object_array(1, jni_str!("java/lang/String"), &gcm)?;
    env.call_method(
        &builder,
        jni_str!("setBlockModes"),
        jni_sig!("([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;"),
        &[JValue::Object(&modes)],
    )?;
    let padding = env.new_string("NoPadding")?;
    let paddings = env.new_object_array(1, jni_str!("java/lang/String"), &padding)?;
    env.call_method(
        &builder,
        jni_str!("setEncryptionPaddings"),
        jni_sig!("([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;"),
        &[JValue::Object(&paddings)],
    )?;
    env.call_method(
        &builder,
        jni_str!("setKeySize"),
        jni_sig!("(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;"),
        &[JValue::Int(256)],
    )?;
    let spec = env
        .call_method(
            &builder,
            jni_str!("build"),
            jni_sig!("()Landroid/security/keystore/KeyGenParameterSpec;"),
            &[],
        )?
        .l()?;
    env.call_method(
        &generator,
        jni_str!("init"),
        jni_sig!("(Ljava/security/spec/AlgorithmParameterSpec;)V"),
        &[JValue::Object(&spec)],
    )?;
    Ok(env
        .call_method(
            &generator,
            jni_str!("generateKey"),
            jni_sig!("()Ljavax/crypto/SecretKey;"),
            &[],
        )?
        .l()?)
}

fn cipher<'local>(
    env: &mut Env<'local>,
    port: u16,
    iv: Option<&[u8]>,
) -> Result<JObject<'local>, StorageError> {
    let transformation = env.new_string("AES/GCM/NoPadding")?;
    let cipher = env
        .call_static_method(
            jni_str!("javax/crypto/Cipher"),
            jni_str!("getInstance"),
            jni_sig!("(Ljava/lang/String;)Ljavax/crypto/Cipher;"),
            &[JValue::Object(&transformation)],
        )?
        .l()?;
    let key = key(env, iv.is_none())?;
    if let Some(iv) = iv {
        let iv = env.byte_array_from_slice(iv)?;
        let spec = env.new_object(
            jni_str!("javax/crypto/spec/GCMParameterSpec"),
            jni_sig!("(I[B)V"),
            &[
                JValue::Int((session_record::TAG_BYTES * 8) as i32),
                JValue::Object(&iv),
            ],
        )?;
        env.call_method(
            &cipher,
            jni_str!("init"),
            jni_sig!("(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V"),
            &[JValue::Int(2), JValue::Object(&key), JValue::Object(&spec)],
        )?;
    } else {
        // Android Keystore chooses the fresh encryption IV; never reuse a caller IV.
        env.call_method(
            &cipher,
            jni_str!("init"),
            jni_sig!("(ILjava/security/Key;)V"),
            &[JValue::Int(1), JValue::Object(&key)],
        )?;
    }
    let aad = env.byte_array_from_slice(&session_record::aad(port)?)?;
    env.call_method(
        &cipher,
        jni_str!("updateAAD"),
        jni_sig!("([B)V"),
        &[JValue::Object(&aad)],
    )?;
    Ok(cipher)
}

fn bytes(env: &Env<'_>, object: JObject<'_>) -> Result<Vec<u8>, StorageError> {
    let array = env.cast_local::<JByteArray>(object)?;
    let bytes = env.convert_byte_array(&array)?;
    array.set_region(env, 0, &vec![0; bytes.len()])?;
    Ok(bytes)
}

fn crypt(env: &mut Env<'_>, cipher: &JObject<'_>, input: &[u8]) -> Result<Vec<u8>, StorageError> {
    let length = input.len();
    let input = env.byte_array_from_slice(input)?;
    let output = env
        .call_method(
            cipher,
            jni_str!("doFinal"),
            jni_sig!("([B)[B"),
            &[JValue::Object(&input)],
        )?
        .l()?;
    input.set_region(env, 0, &vec![0; length])?;
    bytes(env, output)
}

/// Read only the selected origin. Missing is not an error; corruption is never discarded.
pub(crate) fn load(
    env: &mut Env<'_>,
    context: &JObject<'_>,
    port: u16,
) -> Result<Option<String>, StorageError> {
    let _guard = ACCESS.lock().map_err(|_| StorageError::Lock)?;
    let Some((iv, sealed)) = session_record::load(&record(env, context, port)?)? else {
        return Ok(None);
    };
    let cipher = cipher(env, port, Some(&iv))?;
    let mut plain = crypt(env, &cipher, &sealed)?;
    let token = if session_record::valid_token(&plain) {
        String::from_utf8(plain.clone())
            .map_err(|_| StorageError::Invalid)
            .map(Some)
    } else {
        Err(StorageError::Invalid)
    };
    plain.fill(0);
    token
}

/// Sync and atomically replace the same legacy-compatible encrypted no_backup record.
pub(crate) fn save(
    env: &mut Env<'_>,
    context: &JObject<'_>,
    port: u16,
    token: &str,
) -> Result<(), StorageError> {
    let _guard = ACCESS.lock().map_err(|_| StorageError::Lock)?;
    if !session_record::valid_token(token.as_bytes()) {
        return Err(StorageError::Invalid);
    }
    let file = record(env, context, port)?;
    let cipher = cipher(env, port, None)?;
    let iv_object = env
        .call_method(&cipher, jni_str!("getIV"), jni_sig!("()[B"), &[])?
        .l()?;
    let iv = bytes(env, iv_object)?;
    let sealed = crypt(env, &cipher, token.as_bytes())?;
    session_record::save(&file, &iv, &sealed)?;
    Ok(())
}

/// Remove only this origin and its pending write after rejection or acknowledged logout.
pub(crate) fn forget(
    env: &mut Env<'_>,
    context: &JObject<'_>,
    port: u16,
) -> Result<(), StorageError> {
    let _guard = ACCESS.lock().map_err(|_| StorageError::Lock)?;
    session_record::forget(&record(env, context, port)?)?;
    Ok(())
}
