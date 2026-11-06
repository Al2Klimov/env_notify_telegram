use std::env::var_os;
use std::ffi::OsString;
use std::str::Utf8Error;
use teloxide_core::Bot;

pub(crate) enum Action {
    ListChats,
    SendMessage { chat_id: String, message: String },
}

pub(crate) struct EnvFailure {
    pub(crate) var: &'static str,
    pub(crate) err: EnvError,
}

pub(crate) enum EnvError {
    Missing,
    Empty,
    BadUnicode(Utf8Error),
}

pub(crate) fn parse_env() -> Result<(Bot, Action), EnvFailure> {
    Ok((
        Bot::new(require_env_notempty_utf8("ENTG_TOKEN")?),
        match env_notempty_utf8("ENTG_CHAT")? {
            None => Action::ListChats,
            Some(chat_id) => Action::SendMessage {
                chat_id,
                message: require_env("ENTG_MESSAGE")?.to_string_lossy().to_string(),
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
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clean_env() {
        env::remove_var("ENTG_TOKEN");
        env::remove_var("ENTG_CHAT");
        env::remove_var("ENTG_MESSAGE");
    }

    #[test]
    fn test_missing_token() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();

        let result = parse_env();

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
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();
        env::set_var("ENTG_TOKEN", "");

        let result = parse_env();

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
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();
        env::set_var("ENTG_TOKEN", "testtoken");

        let result = parse_env();

        assert!(matches!(result, Ok((bot, Action::ListChats)) if bot.token() == "testtoken"));
    }

    #[test]
    fn test_valid_token_empty_chat() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();
        env::set_var("ENTG_TOKEN", "testtoken");
        env::set_var("ENTG_CHAT", "");

        let result = parse_env();

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
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();
        env::set_var("ENTG_TOKEN", "testtoken");
        env::set_var("ENTG_CHAT", "123");

        let result = parse_env();

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
        let _guard = ENV_MUTEX.lock().unwrap();
        clean_env();
        env::set_var("ENTG_TOKEN", "testtoken");
        env::set_var("ENTG_CHAT", "123");
        env::set_var("ENTG_MESSAGE", "Hello, World!");

        let result = parse_env();

        assert!(
            matches!(result, Ok((bot, Action::SendMessage{chat_id, message: _})) if bot.token() == "testtoken" && chat_id == "123")
        );
    }
}
