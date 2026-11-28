use redis::{Client, IntoConnectionInfo, RedisError};
use std::env::var_os;
use std::ffi::OsString;
use std::str::Utf8Error;
use teloxide_core::Bot;

pub(crate) enum Action {
    ListChats,
    SendMessage {
        chat_id: String,
        message: String,
        mute: Option<(Vec<u8>, Client)>,
    },
}

pub(crate) struct EnvFailure {
    pub(crate) var: &'static str,
    pub(crate) err: EnvError,
}

pub(crate) enum EnvError {
    Missing,
    Empty,
    BadUnicode(Utf8Error),
    BadRedis(RedisError),
}

pub(crate) fn parse_env() -> Result<(Bot, Action), EnvFailure> {
    Ok((
        Bot::new(require_env_notempty_utf8("ENTG_TOKEN")?),
        match env_notempty_utf8("ENTG_CHAT")? {
            None => Action::ListChats,
            Some(chat_id) => Action::SendMessage {
                chat_id,
                message: require_env("ENTG_MESSAGE")?.to_string_lossy().to_string(),
                mute: match var_os("ENTG_SERVICE") {
                    None => None,
                    Some(v) => Some((v.into_encoded_bytes(), require_redis("ENTG_REDIS")?)),
                },
            },
        },
    ))
}

fn require_env(var: &'static str) -> Result<OsString, EnvFailure> {
    process_env(var, env_present)
}

fn env_notempty_utf8(var: &'static str) -> Result<Option<String>, EnvFailure> {
    process_env(var, |k, v| {
        v.map(|v| env_notempty(k, v).and_then(|v| env_utf8(k, v)))
            .transpose()
    })
}

fn require_redis(var: &'static str) -> Result<Client, EnvFailure> {
    require_env_notempty_utf8(var)?
        .into_connection_info()
        .map(|info| {
            let settings = info.redis_settings().clone().set_skip_set_lib_name();
            info.set_redis_settings(settings)
        })
        .and_then(Client::open)
        .map_err(|err| EnvFailure {
            var,
            err: EnvError::BadRedis(err),
        })
}

fn require_env_notempty_utf8(var: &'static str) -> Result<String, EnvFailure> {
    process_env(var, |k, v| {
        env_present(k, v)
            .and_then(|v| env_notempty(k, v))
            .and_then(|v| env_utf8(k, v))
    })
}

fn process_env<T>(var: &'static str, process: fn(&'static str, Option<OsString>) -> T) -> T {
    process(var, var_os(var))
}

fn env_present(var: &'static str, value: Option<OsString>) -> Result<OsString, EnvFailure> {
    value.ok_or(EnvFailure {
        var,
        err: EnvError::Missing,
    })
}

fn env_notempty(var: &'static str, value: OsString) -> Result<OsString, EnvFailure> {
    if value.is_empty() {
        Err(EnvFailure {
            var,
            err: EnvError::Empty,
        })
    } else {
        Ok(value)
    }
}

fn env_utf8(var: &'static str, value: OsString) -> Result<String, EnvFailure> {
    String::from_utf8(value.into_encoded_bytes()).map_err(|err| EnvFailure {
        var,
        err: EnvError::BadUnicode(err.utf8_error()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env::with_vars;

    fn with_env<R>(
        token: Option<&str>,
        chat: Option<&str>,
        message: Option<&str>,
        f: impl FnOnce() -> R,
    ) -> R {
        with_vars(
            [
                ("ENTG_TOKEN", token),
                ("ENTG_CHAT", chat),
                ("ENTG_MESSAGE", message),
            ],
            f,
        )
    }

    #[test]
    fn test_missing_token() {
        let result = with_env(None, None, None, parse_env);

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_TOKEN",
                err: EnvError::Missing,
            })
        ));
    }

    #[test]
    fn test_empty_token() {
        let result = with_env(Some(""), None, None, parse_env);

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_TOKEN",
                err: EnvError::Empty,
            })
        ));
    }

    #[test]
    fn test_valid_token_no_chat() {
        let result = with_env(Some("testtoken"), None, None, parse_env);

        assert!(matches!(result, Ok((bot, Action::ListChats)) if bot.token() == "testtoken"));
    }

    #[test]
    fn test_valid_token_empty_chat() {
        let result = with_env(Some("testtoken"), Some(""), None, parse_env);

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_CHAT",
                err: EnvError::Empty,
            })
        ));
    }

    #[test]
    fn test_valid_token_and_chat_no_message() {
        let result = with_env(Some("testtoken"), Some("123"), None, parse_env);

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_MESSAGE",
                err: EnvError::Missing,
            })
        ));
    }

    #[test]
    fn test_all_env_vars_set() {
        let result = with_env(
            Some("testtoken"),
            Some("123"),
            Some("Hello, World!"),
            parse_env,
        );

        assert!(
            matches!(result, Ok((bot, Action::SendMessage{chat_id, message: _, mute: _})) if bot.token() == "testtoken" && chat_id == "123")
        );
    }

    fn with_full_env<R>(vars: [(&str, Option<&str>); 5], f: impl FnOnce() -> R) -> R {
        with_vars(vars, f)
    }

    #[cfg(unix)]
    #[test]
    fn test_bad_unicode_token() {
        use std::os::unix::ffi::OsStringExt;

        let bad = OsString::from_vec(vec![0x66, 0xff, 0x6f]);
        let result = with_vars(
            [
                ("ENTG_TOKEN", Some(bad)),
                ("ENTG_CHAT", None),
                ("ENTG_MESSAGE", None),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_TOKEN",
                err: EnvError::BadUnicode(_),
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_bad_unicode_chat() {
        use std::os::unix::ffi::OsStringExt;

        let bad = OsString::from_vec(vec![0xff]);
        let result = with_vars(
            [
                ("ENTG_TOKEN", Some(OsString::from("testtoken"))),
                ("ENTG_CHAT", Some(bad)),
                ("ENTG_MESSAGE", None),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_CHAT",
                err: EnvError::BadUnicode(_),
            })
        ));
    }

    #[test]
    fn test_message_content() {
        let result = with_env(Some("t"), Some("123"), Some("Hello"), parse_env);

        assert!(matches!(
            result,
            Ok((_, Action::SendMessage { message, mute: None, .. })) if message == "Hello"
        ));
    }

    #[test]
    fn test_service_without_redis() {
        let result = with_full_env(
            [
                ("ENTG_TOKEN", Some("t")),
                ("ENTG_CHAT", Some("123")),
                ("ENTG_MESSAGE", Some("m")),
                ("ENTG_SERVICE", Some("svc")),
                ("ENTG_REDIS", None),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_REDIS",
                err: EnvError::Missing,
            })
        ));
    }

    #[test]
    fn test_service_with_empty_redis() {
        let result = with_full_env(
            [
                ("ENTG_TOKEN", Some("t")),
                ("ENTG_CHAT", Some("123")),
                ("ENTG_MESSAGE", Some("m")),
                ("ENTG_SERVICE", Some("svc")),
                ("ENTG_REDIS", Some("")),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_REDIS",
                err: EnvError::Empty,
            })
        ));
    }

    #[test]
    fn test_service_with_bad_redis() {
        let result = with_full_env(
            [
                ("ENTG_TOKEN", Some("t")),
                ("ENTG_CHAT", Some("123")),
                ("ENTG_MESSAGE", Some("m")),
                ("ENTG_SERVICE", Some("svc")),
                ("ENTG_REDIS", Some("http://localhost")),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Err(EnvFailure {
                var: "ENTG_REDIS",
                err: EnvError::BadRedis(_),
            })
        ));
    }

    #[test]
    fn test_service_with_valid_redis() {
        let result = with_full_env(
            [
                ("ENTG_TOKEN", Some("t")),
                ("ENTG_CHAT", Some("123")),
                ("ENTG_MESSAGE", Some("m")),
                ("ENTG_SERVICE", Some("svc")),
                ("ENTG_REDIS", Some("redis://localhost")),
            ],
            parse_env,
        );

        assert!(matches!(
            result,
            Ok((_, Action::SendMessage { mute: Some((svc, _)), .. })) if svc == b"svc"
        ));
    }
}
