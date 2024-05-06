use std::error::Error;

use axum::body::Bytes;
use once_cell::sync::Lazy;
use serde::Serialize;
use teloxide::{
    requests::Requester,
    types::{ChatId, InputFile, MessageId, Recipient},
};
use tokio::fs::File;
use tracing::log;
use moka::future::Cache;

use super::bot::ROUND_ROBIN_BOT;
use crate::config::CONFIG;

#[derive(Serialize)]
pub struct UploadedFile {
    pub backend: String,
    pub data: MessageInfo,
}

#[derive(Serialize)]
pub struct MessageInfo {
    pub chat_id: i64,
    pub message_id: i32,
}

pub static TEMP_FILES_CACHE: Lazy<Cache<i32, MessageId>> = Lazy::new(|| {
    Cache::builder()
        .time_to_idle(std::time::Duration::from_secs(5 * 60 * 60))
        .max_capacity(4098)
        .async_eviction_listener(|_data_id, message_id, _cause| {
            Box::pin(async move {
                let bot = ROUND_ROBIN_BOT.get_bot();
                let _ = bot
                    .delete_message(
                        Recipient::Id(ChatId(CONFIG.telegram_temp_chat_id)),
                        message_id,
                    )
                    .await;
            })
        })
        .build()
});

pub async fn upload_file(
    file: Bytes,
    filename: String,
    caption: Option<String>,
) -> Result<UploadedFile, String> {
    let bot = ROUND_ROBIN_BOT.get_bot();
    let document = InputFile::memory(file).file_name(filename);

    let mut request = bot.send_document(ChatId(CONFIG.telegram_chat_id), document);
    request.caption = caption;

    let result = request.await;

    match result {
        Ok(message) => Ok(UploadedFile {
            backend: "bot".to_string(),
            data: MessageInfo {
                chat_id: message.chat.id.0,
                message_id: message.id.0,
            },
        }),
        Err(err) => Err(err.to_string()),
    }
}

pub async fn download_file(chat_id: i64, message_id: i32) -> Option<BotDownloader> {
    let bot = ROUND_ROBIN_BOT.get_bot();

    let forwarded_message = match bot
        .forward_message(
            ChatId(CONFIG.telegram_temp_chat_id),
            ChatId(chat_id),
            MessageId(message_id),
        )
        .await
    {
        Ok(v) => v,
        Err(err) => {
            log::error!("Error: {}", err);
            return None;
        }
    };

    let file_id = match forwarded_message.document() {
        Some(v) => v.file.id.clone(),
        None => {
            log::error!("Document not found!");
            return None;
        }
    };

    TEMP_FILES_CACHE.insert(message_id, forwarded_message.id.clone()).await;

    let path = match bot.get_file(file_id.clone()).await {
        Ok(v) => v.path,
        Err(err) => {
            log::error!("Error: {}", err);
            return None;
        }
    };

    log::info!("File path: {}", path);

    return Some(BotDownloader(path));
}

pub struct BotDownloader(String);

impl BotDownloader {
    pub async fn get_file(self) -> Result<File, Box<dyn Error>> {
        Ok(File::open(self.0).await?)
    }
}
