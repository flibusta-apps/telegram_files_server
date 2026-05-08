use std::time::Duration;

use axum::body::Bytes;
use moka::future::Cache;
use once_cell::sync::Lazy;
use serde::Serialize;
use teloxide::{
    requests::Requester,
    types::{ChatId, InputFile, MessageId, Recipient},
};
use tokio::fs::File;
use tracing::log;

use super::bot::ROUND_ROBIN_BOT;
use crate::config::CONFIG;
use crate::core::errors::FileError;

const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const TELEGRAM_API_TIMEOUT: Duration = Duration::from_secs(30);

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
        .time_to_idle(Duration::from_secs(16))
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

/// Returns true if the error is transient and worth retrying (network errors only).
/// Rate limit errors (RetryAfter) are handled separately in retry_async.
fn is_transient_error(err: &teloxide::RequestError) -> bool {
    matches!(err, teloxide::RequestError::Network(_))
}

/// Returns true if the error means the file/message doesn't exist and never will.
fn is_not_found_error(err: &teloxide::RequestError) -> bool {
    match err {
        teloxide::RequestError::Api(api_err) => {
            matches!(
                api_err,
                teloxide::ApiError::MessageToForwardNotFound
                    | teloxide::ApiError::MessageIdInvalid
                    | teloxide::ApiError::WrongFileId
                    | teloxide::ApiError::WrongFileIdOrUrl
                    | teloxide::ApiError::FileIdInvalid
            ) || matches!(api_err, teloxide::ApiError::Unknown(msg) if is_wrong_file_id_unknown(msg))
        }
        _ => false,
    }
}

/// Telegram returns "Bad Request: wrong file_id or the file is temporarily unavailable"
/// as an Unknown variant because teloxide doesn't have a specific enum member for it.
fn is_wrong_file_id_unknown(msg: &str) -> bool {
    msg.contains("wrong file_id") || msg.contains("temporarily unavailable")
}

pub async fn upload_file(
    file: Bytes,
    filename: String,
    caption: Option<String>,
    chat_id: i64,
) -> Result<UploadedFile, FileError> {
    let bot = ROUND_ROBIN_BOT.get_bot();
    let document = InputFile::memory(file).file_name(filename);

    let mut request = bot.send_document(ChatId(chat_id), document);
    request.caption = caption;

    let result = tokio::time::timeout(TELEGRAM_API_TIMEOUT, request)
        .await
        .map_err(|_| FileError::FileUnavailable("Telegram API timeout on upload".to_string()))?
        .map_err(|err| {
            if is_not_found_error(&err) {
                FileError::FileUnavailable(err.to_string())
            } else if let teloxide::RequestError::RetryAfter(secs) = &err {
                FileError::RateLimited(secs.seconds() as u64)
            } else {
                FileError::TelegramApi(err)
            }
        })?;

    Ok(UploadedFile {
        backend: "bot".to_string(),
        data: MessageInfo {
            chat_id: result.chat.id.0,
            message_id: result.id.0,
        },
    })
}

pub async fn download_file(chat_id: i64, message_id: i32) -> Result<Option<File>, FileError> {
    let forwarded_message = retry_async(|| {
        Box::pin(async move {
            let bot = ROUND_ROBIN_BOT.get_bot();
            let result = tokio::time::timeout(
                TELEGRAM_API_TIMEOUT,
                bot.forward_message(
                    ChatId(CONFIG.telegram_temp_chat_id),
                    ChatId(chat_id),
                    MessageId(message_id),
                ),
            )
            .await;

            match result {
                Ok(Ok(msg)) => Ok(msg),
                Ok(Err(err)) => {
                    if is_not_found_error(&err) {
                        Err(FileError::FileUnavailable(err.to_string()))
                    } else if let teloxide::RequestError::RetryAfter(secs) = &err {
                        Err(FileError::RateLimited(secs.seconds() as u64))
                    } else {
                        Err(FileError::TelegramApi(err))
                    }
                }
                Err(_) => Err(FileError::FileUnavailable(
                    "Telegram API timeout on forward_message".to_string(),
                )),
            }
        })
    })
    .await?;

    // If the message has no document, treat as not found
    let document = match forwarded_message.document() {
        Some(doc) => doc,
        None => return Ok(None),
    };
    let file_id = document.file.id.clone();

    TEMP_FILES_CACHE
        .insert(message_id, forwarded_message.id)
        .await;

    let path = retry_async(|| {
        let file_id = file_id.clone();
        Box::pin(async move {
            let bot = ROUND_ROBIN_BOT.get_bot();
            let result = tokio::time::timeout(TELEGRAM_API_TIMEOUT, bot.get_file(file_id)).await;

            match result {
                Ok(Ok(file)) => Ok(file.path),
                Ok(Err(err)) => {
                    if is_not_found_error(&err) {
                        Err(FileError::FileUnavailable(err.to_string()))
                    } else if let teloxide::RequestError::RetryAfter(secs) = &err {
                        Err(FileError::RateLimited(secs.seconds() as u64))
                    } else {
                        Err(FileError::TelegramApi(err))
                    }
                }
                Err(_) => Err(FileError::FileUnavailable(
                    "Telegram API timeout on get_file".to_string(),
                )),
            }
        })
    })
    .await?;

    Ok(Some(File::open(path).await?))
}

/// Retry an async operation with exponential backoff.
/// Only retries on transient errors (network, rate limit).
/// On rate limit, respects the retry-after hint.
async fn retry_async<F, Fut, T>(f: F) -> Result<T, FileError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, FileError>>,
{
    let mut attempt = 0;
    let mut delay = INITIAL_RETRY_DELAY;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(FileError::RateLimited(secs)) => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    return Err(FileError::RateLimited(secs));
                }
                tokio::time::sleep(Duration::from_secs(secs)).await;
            }
            Err(FileError::TelegramApi(ref err)) if is_transient_error(err) => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    return Err(FileError::TelegramApi(err.clone()));
                }
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(other) => return Err(other),
        }
    }
}

pub async fn clean_files() -> Result<(), FileError> {
    let bots_folder = "/var/lib/telegram-bot-api/";
    let documents_folder_name = "documents";

    let mut bots_folder = tokio::fs::read_dir(bots_folder).await?;

    while let Some(entry) = bots_folder.next_entry().await? {
        if !entry.metadata().await?.is_dir() {
            continue;
        }

        let documents_folder_path = entry.path().join(documents_folder_name);
        if !documents_folder_path.exists() {
            continue;
        }

        let mut document_folder = match tokio::fs::read_dir(documents_folder_path.clone()).await {
            Ok(v) => v,
            Err(err) => {
                log::error!("Path: {:?}, Error: {:?}", documents_folder_path, err);
                continue;
            }
        };

        while let Some(file) = document_folder.next_entry().await? {
            let metadata = file.metadata().await?;

            if metadata.created()?.elapsed()?.as_secs() > 3600 {
                match tokio::fs::remove_file(file.path()).await {
                    Ok(_) => log::info!("File {:?} removed", file.path()),
                    Err(err) => log::error!("Error: {}", err),
                }
            }
        }
    }

    Ok(())
}
