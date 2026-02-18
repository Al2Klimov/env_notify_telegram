use std::env::var_os;
use std::ffi::OsString;
use std::str::Utf8Error;

pub(crate) enum Action {
    ListChats(String),
    SendMessage(String, String, OsString),
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

pub(crate) fn parse_env() -> Result<Action, EnvFailure> {
    let token = require_noempty_utf8_env("ENTG_TOKEN")?;
    match require_noempty_utf8_env("ENTG_CHAT") {
        Err(err) => match err.err {
            EnvError::Missing => Ok(Action::ListChats(token)),
            _ => Err(err),
        },
        Ok(chat) => Ok(Action::SendMessage(
            token,
            chat,
            require_env("ENTG_MESSAGE")?,
        )),
    }
}

fn require_noempty_utf8_env(var: &'static str) -> Result<String, EnvFailure> {
    let oss = require_env(var)?;
    if oss.is_empty() {
        Err(EnvError::Empty)
    } else {
        String::from_utf8(oss.into_encoded_bytes())
            .map_err(|err| EnvError::BadUnicode(err.utf8_error()))
    }
    .map_err(|err| EnvFailure { var, err })
}

fn require_env(var: &'static str) -> Result<OsString, EnvFailure> {
    match var_os(var) {
        None => Err(EnvFailure {
            var,
            err: EnvError::Missing,
        }),
        Some(oss) => Ok(oss),
    }
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

        assert!(matches!(result, Ok(Action::ListChats(t)) if t == "testtoken"));
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
            matches!(result, Ok(Action::SendMessage(t, c, _)) if t == "testtoken" && c == "123")
        );
    }
}
