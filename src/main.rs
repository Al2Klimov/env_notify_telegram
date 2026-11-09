mod cli;

use crate::cli::Action;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cli::EnvError;
use humantime::format_duration;
use redis::{AsyncTypedCommands, Client, Pipeline};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};
use std::cmp::{max, min};
use std::process::exit;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide_core::payloads::{GetUpdatesSetters, SendMessageSetters};
use teloxide_core::requests::Requester;
use teloxide_core::types::{
    AllowedUpdate, InlineKeyboardButton, InlineKeyboardMarkup, Message, UpdateKind,
};
use teloxide_core::Bot;
use tokio::join;

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum Status {
    SendSuccess = 0,
    MuteFailure = 1,
    SendFailure = 2,
    NothingToSend = 3,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    match cli::parse_env() {
        Err(err) => {
            eprint!("Environment variable {} ", err.var);

            match err.err {
                EnvError::Missing => eprintln!("missing"),
                EnvError::Empty => eprintln!("is empty"),
                EnvError::BadUnicode(err) => eprintln!("is not valid UTF-8: {}", err),
                EnvError::BadRedis(err) => eprintln!("is not a valid Redis URL: {}", err),
            }

            exit(Status::NothingToSend as i32);
        }
        Ok((bot, action)) => match action {
            Action::ListChats => {
                eprintln!("No chat given, discovering existing chats...");

                match bot
                    .get_updates()
                    .allowed_updates(vec![AllowedUpdate::Message])
                    .await
                {
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

                exit(Status::NothingToSend as i32);
            }
            Action::SendMessage {
                chat_id,
                message,
                mute,
            } => {
                let mut status = Status::SendSuccess;
                let mut msg = bot.send_message(chat_id.clone(), message);

                match mute {
                    None => {}
                    Some((svcid, redis)) => {
                        let (muted, state) = handle_mute(&bot, redis, &chat_id, &svcid).await;
                        status = max(status, state);

                        if muted {
                            exit(status as i32);
                        }

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        msg = msg.reply_markup(InlineKeyboardMarkup::new(vec![
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
                        ]));
                    }
                };

                status = max(
                    status,
                    match msg.await {
                        Err(err) => {
                            eprintln!("{}", err);
                            Status::SendFailure
                        }
                        Ok(_) => Status::SendSuccess,
                    },
                );

                exit(status as i32);
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

async fn handle_mute(
    bot: &Bot,
    redis: Client,
    chat_id: &String,
    svcid: &Vec<u8>,
) -> (bool, Status) {
    macro_rules! fatal {
        ($result:expr) => {
            match $result {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("{}", err);
                    return (false, Status::MuteFailure);
                }
            }
        };
    }

    macro_rules! not_fatal {
        ($result:expr, $status:ident, $ok_var:ident, $ok_expr:expr) => {
            match $result {
                Err(err) => {
                    eprintln!("{}", err);
                    $status = max($status, Status::MuteFailure);
                }
                Ok($ok_var) => $ok_expr,
            }
        };
    }

    let (r_rcon, r_bot_info) = join!(redis.get_multiplexed_async_connection(), bot.get_me());
    let mut rcon = fatal!(r_rcon);
    let bot_id = fatal!(r_bot_info).id.0;

    let offset_key = format!("{{env_notify_telegram:v1:{}}}:offset", bot_id);
    let mut status = Status::SendSuccess;

    let mut updates = bot
        .get_updates()
        .allowed_updates(vec![AllowedUpdate::CallbackQuery, AllowedUpdate::Message]);

    not_fatal!(
        rcon.get(&offset_key).await,
        status,
        o_offset,
        match o_offset {
            None => {}
            Some(s_offset) => not_fatal!(
                s_offset.parse::<i32>(),
                status,
                offset,
                updates = updates.offset(offset + 1)
            ),
        }
    );

    let mut off_all = None;
    let mut off_msg = None;

    not_fatal!(updates.await, status, resp, {
        let mut pipe = Pipeline::new();
        pipe.atomic();

        for update in resp {
            off_all = Some(match off_all {
                None => update.id.0,
                Some(off) => max(off, update.id.0),
            });

            match update.kind {
                UpdateKind::Message(_) => {
                    off_msg = Some(match off_msg {
                        None => update.id.0,
                        Some(off) => min(off, update.id.0),
                    });
                }
                UpdateKind::CallbackQuery(query) => match query.data {
                    None => {}
                    Some(data) => not_fatal!(
                        serde_json::from_str::<TelegramCallback>(data.as_str()),
                        status,
                        tcb,
                        match tcb {
                            TelegramCallback::MuteV1(mute) => {
                                let key = format!(
                                    "{{env_notify_telegram:v1:{}}}:mute:{}:{}",
                                    bot_id,
                                    query.from.id.0,
                                    STANDARD.encode(mute.service)
                                );

                                pipe.set(&key, "");
                                pipe.expire_at(key, mute.until);
                            }
                        }
                    ),
                },
                _ => {}
            }
        }

        match off_all {
            None => {}
            Some(offall) => {
                pipe.set(
                    offset_key,
                    match off_msg {
                        Some(offmsg) => offmsg - 1,
                        None => offall,
                    },
                );

                not_fatal!(pipe.exec_async(&mut rcon).await, status, __, {});
            }
        }
    });

    let muted = fatal!(
        rcon.get(format!(
            "{{env_notify_telegram:v1:{}}}:mute:{}:{}",
            bot_id,
            chat_id,
            STANDARD.encode(svcid)
        ))
        .await
    );

    (muted.is_some(), status)
}

fn mute_for(svcid: &Vec<u8>, now: i64, hours: i64, label: &'static str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(
        label,
        serde_json::to_string(&TelegramCallback::MuteV1(MuteV1 {
            service: svcid.clone(),
            until: now + hours * 3600,
        }))
        .unwrap_or_default(),
    )
}

#[derive(Deserialize, Serialize)]
enum TelegramCallback {
    MuteV1(MuteV1),
}

#[serde_as]
#[derive(Deserialize, Serialize)]
struct MuteV1 {
    #[serde_as(as = "Base64")]
    service: Vec<u8>,
    until: i64,
}
