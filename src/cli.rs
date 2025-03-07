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
