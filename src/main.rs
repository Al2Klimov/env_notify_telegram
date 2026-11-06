mod cli;

use crate::cli::Action;
use cli::EnvError;
use humantime::format_duration;
use serde::Serialize;
use serde_with::{base64::Base64, serde_as};
use std::process::exit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide_core::payloads::SendMessageSetters;
use teloxide_core::requests::Requester;
use teloxide_core::types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, UpdateKind};

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
                                UpdateKind::CallbackQuery(query) => eprintln!(
                                    "{}",
                                    serde_json::to_string_pretty(&query).unwrap_or_default()
                                ),
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
            Action::SendMessage {
                chat_id,
                service,
                message,
            } => {
                let msg = bot.send_message(chat_id, message);
                let muteable = match service {
                    None => msg,
                    Some(svcid) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        msg.reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![
                                mute_for(&svcid, now, 1, "🔇1h"),
                                mute_for(&svcid, now, 2, "2"),
                                mute_for(&svcid, now, 3, "3"),
                                mute_for(&svcid, now, 5, "5"),
                                mute_for(&svcid, now, 8, "8"),
                                mute_for(&svcid, now, 13, "13🕒"),
                            ],
                            vec![
                                mute_for(&svcid, now, 1 * 24, "🔇1d"),
                                mute_for(&svcid, now, 2 * 24, "2"),
                                mute_for(&svcid, now, 3 * 24, "3"),
                                mute_for(&svcid, now, 5 * 24, "5"),
                                mute_for(&svcid, now, 8 * 24, "8"),
                                mute_for(&svcid, now, 13 * 24, "13📅"),
                            ],
                        ]))
                    }
                };
                match muteable.await {
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

fn mute_for(svcid: &Vec<u8>, now: u64, hours: u64, label: &'static str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(
        label,
        serde_json::to_string(&TelegramCallback::MuteV1(MuteV1 {
            service: svcid.clone(),
            until: now + hours * 3600,
        }))
        .unwrap_or_default(),
    )
}

#[derive(Serialize)]
enum TelegramCallback {
    MuteV1(MuteV1),
}

#[serde_as]
#[derive(Serialize)]
struct MuteV1 {
    #[serde_as(as = "Base64")]
    service: Vec<u8>,
    until: u64,
}
