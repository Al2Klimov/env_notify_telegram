mod cli;

use crate::cli::Action;
use cli::EnvError;
use humantime::format_duration;
use std::process::exit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide_core::requests::Requester;
use teloxide_core::types::{Message, UpdateKind};

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
        Ok((bot, action)) => match action {
            Action::ListChats => {
                eprintln!("No chat given, discovering existing chats...");

                match bot.get_updates().await {
                    Err(err) => eprintln!("{}", err),
                    Ok(resp) => {
                        let mut messages: usize = 0;
                        let now = SystemTime::now();

                        for update in resp {
                            match update.kind {
                                UpdateKind::Message(message) => {
                                    print_message(message, now);
                                    messages += 1;
                                }
                                _ => {}
                            }
                        }

                        if messages == 0 {
                            eprintln!("No messages found.");
                        }
                    }
                }

                exit(3);
            }
            Action::SendMessage(chat_id, message) => {
                match bot.send_message(chat_id, message).await {
                    Err(err) => {
                        eprintln!("{}", err);
                        exit(2);
                    }
                    Ok(_) => {}
                }
            }
        },
    }
}

fn print_message(message: Message, now: SystemTime) {
    println!(
        "[ENTG_CHAT={}] {} ago: {}: {}",
        message.chat.id,
        format_duration(Duration::from_secs(
            now.duration_since(UNIX_EPOCH + Duration::from_secs(message.date.timestamp() as u64))
                .unwrap_or_default()
                .as_secs()
        )),
        match &message.chat.title() {
            Some(title) => title,
            None => match &message.chat.username() {
                Some(username) => username,
                None => "N/A",
            },
        },
        match &message.text() {
            Some(text) => text,
            None => "N/A",
        }
    );
}
