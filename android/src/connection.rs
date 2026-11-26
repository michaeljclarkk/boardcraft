use gpui_mobile::android::jni::{activity, get_string, with_env};
use jni::{jni_sig, jni_str, objects::JValue};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Connection {
    /// None is the explicit browse action, never an inferred missing UUID.
    pub board: Option<Uuid>,
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
            let has_board = has(&board)?;
            let has_port = has(&port)?;
            let action = env
                .call_method(
                    &intent,
                    jni_str!("getAction"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            let browse = !action.is_null() && get_string(env, &action) == "dev.boardcraft.BROWSE";
            if !browse && !has_board && !has_port {
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
            Ok(Some((
                browse,
                has_board,
                (!id.is_null()).then(|| get_string(env, &id)),
                number,
            )))
        })()
        .map_err(|err| {
            env.exception_clear();
            err.to_string()
        })?;
        args.map(|(browse, has_board, id, number)| {
            let board = match (browse, has_board) {
                (true, false) => None,
                (false, true) => Some(
                    id.and_then(|s| Uuid::parse_str(&s).ok())
                        .filter(|id| !id.is_nil())
                        .ok_or("Invalid board")?,
                ),
                _ => return Err("Invalid destination".into()),
            };
            let port = u16::try_from(number)
                .ok()
                .filter(|n| *n != 0)
                .ok_or("Invalid port")?;
            Ok(Connection { board, port })
        })
        .transpose()
    })
}
