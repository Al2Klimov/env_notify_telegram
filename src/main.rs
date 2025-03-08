mod cli;
mod tgapi;

use crate::cli::Action;
use crate::tgapi::{request, Message, Method, Update};
use cli::EnvError;
use humantime::format_duration;
use std::process::exit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    match cli::parse_env() {
        Err(err) => {
            eprint!("Environment variable {} ", err.var);

            match err.err {
                EnvError::Missing => eprintln!("missing"),
                EnvError::Empty => eprintln!("is empty"),
                EnvError::BadUnicode(err) => eprintln!("is not valid UTF-8: {}", err),
            }

            exit(3);
        }
        Ok(action) => match action {
            Action::ListChats(token) => {
                eprintln!("No chat given, discovering existing chats...");

                match request::<Vec<Update>>(Method::Get, token, "getUpdates") {
                    Err(err) => eprintln!("{}", err),
                    Ok(resp) => {
                        let mut messages: usize = 0;
                        let now = SystemTime::now();

                        for update in resp {
                            match update.message {
                                None => {}
                                Some(message) => {
                                    print_message(message, now);
                                    messages += 1;
                                }
                            }
                        }

                        if messages == 0 {
                            eprintln!("No messages found.");
                        }
                    }
                }

                exit(3);
            }
            Action::SendMessage(token, chat, message) => {
                todo!()
            }
        },
    }
}

fn print_message(message: Message, now: SystemTime) {
    println!(
        "[ENTG_CHAT={}] {} ago: {}: {}",
        message.chat.id,
        format_duration(Duration::from_secs(
            now.duration_since(UNIX_EPOCH + Duration::from_secs(message.date))
                .unwrap_or_default()
                .as_secs()
        )),
        match &message.chat.title {
            Some(title) => title.as_str(),
            None => match &message.chat.username {
                Some(username) => username.as_str(),
                None => "N/A",
            },
        },
        match &message.text {
            Some(text) => text.as_str(),
            None => "N/A",
        }
    );
}
