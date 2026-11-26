use gpui_mobile::android::jni::{activity, get_string, with_env};
use jni::{jni_sig, jni_str, objects::JValue};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Connection {
    pub board: Uuid,
    pub port: u16,
}

/// No arguments opens the offline fixture. Partial or malformed arguments
/// must not masquerade as a successful offline load.
pub fn launch_connection() -> Result<Option<Connection>, String> {
    with_env(|env| {
        let activity = activity(env)?;
        let args = (|| -> jni::errors::Result<_> {
            let intent = env
                .call_method(
                    &activity,
                    jni_str!("getIntent"),
                    jni_sig!("()Landroid/content/Intent;"),
                    &[],
                )?
                .l()?;
            let board = env.new_string("board")?;
            let port = env.new_string("port")?;
            let mut has = |key| {
                env.call_method(
                    &intent,
                    jni_str!("hasExtra"),
                    jni_sig!("(Ljava/lang/String;)Z"),
                    &[JValue::Object(key)],
                )?
                .z()
            };
            if !has(&board)? && !has(&port)? {
                return Ok(None);
            }
            let id = env
                .call_method(
                    &intent,
                    jni_str!("getStringExtra"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&board)],
                )?
                .l()?;
            let number = env
                .call_method(
                    &intent,
                    jni_str!("getIntExtra"),
                    jni_sig!("(Ljava/lang/String;I)I"),
                    &[JValue::Object(&port), JValue::Int(0)],
                )?
                .i()?;
            Ok(Some((get_string(env, &id), number)))
        })()
        .map_err(|err| {
            env.exception_clear();
            err.to_string()
        })?;
        args.map(|(id, number)| {
            let board = Uuid::parse_str(&id)
                .ok()
                .filter(|id| !id.is_nil())
                .ok_or("Invalid board")?;
            let port = u16::try_from(number)
                .ok()
                .filter(|n| *n != 0)
                .ok_or("Invalid port")?;
            Ok(Connection { board, port })
        })
        .transpose()
    })
}
