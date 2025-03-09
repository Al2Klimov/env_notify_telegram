use percent_encoding_rfc3986::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use ureq::Error;

#[derive(Deserialize)]
struct Response<T> {
    result: T,
}

#[derive(Deserialize)]
pub(crate) struct Update {
    pub(crate) message: Option<Message>,
}

#[derive(Serialize)]
pub(crate) struct SendMessage {
    pub(crate) chat_id: String,
    pub(crate) text: String,
}

#[derive(Deserialize)]
pub(crate) struct Message {
    pub(crate) date: u64,
    pub(crate) chat: Chat,
    pub(crate) text: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Chat {
    pub(crate) id: isize,
    pub(crate) title: Option<String>,
    pub(crate) username: Option<String>,
}

pub(crate) enum Method<T: Serialize> {
    Get,
    Post(T),
}

pub(crate) struct Failure {
    method: &'static str,
    token_len: usize,
    action: &'static str,
    err: Error,
}

const BOT_API: &str = "https://api.telegram.org/bot";
const CHARS: AsciiSet = NON_ALPHANUMERIC.remove(b':');

pub(crate) fn request<Req, Resp>(
    method: Method<Req>,
    token: String,
    action: &'static str,
) -> Result<Resp, Failure>
where
    Req: Serialize,
    Resp: DeserializeOwned,
{
    let url = format!(
        "{}{}/{}",
        BOT_API,
        utf8_percent_encode(token.as_str(), &CHARS),
        action
    );

    let methd = match method {
        Method::Get => "GET",
        Method::Post(_) => "POST",
    };

    let req = match method {
        Method::Get => ureq::get(url).call(),
        Method::Post(body) => ureq::post(url).send_json(body),
    };

    match req {
        Err(err) => Err(err),
        Ok(mut resp) => resp.body_mut().read_json::<Response<Resp>>(),
    }
    .map(|r| r.result)
    .map_err(|err| Failure {
        method: methd,
        token_len: token.len(),
        action,
        err,
    })
}

impl Display for Failure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method, BOT_API)?;

        for _ in 0..self.token_len {
            write!(f, "*")?;
        }

        writeln!(f, "/{}: {}", self.action, self.err)
    }
}
