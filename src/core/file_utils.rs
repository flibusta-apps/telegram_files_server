use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use serde::Serialize;
use teloxide::{
    requests::{Request, Requester},
    types::{ChatId, InputFile, MessageId, Recipient},
    Bot, RequestError,
};
use tempfile::{NamedTempFile, TempPath};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::log;

use super::bot::ROUND_ROBIN_BOT;
use crate::config::CONFIG;
use crate::core::errors::FileError;

/// Number of retries after the first attempt (so up to `MAX_RETRIES + 1` attempts in total).
const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const TELEGRAM_API_TIMEOUT: Duration = Duration::from_secs(30);
/// Telegram RetryAfter up to this value is waited out internally; longer ones are returned as 429.
const MAX_INTERNAL_RETRY_AFTER_SECS: u64 = 5;
/// How long a forwarded temporary message lives before it is deleted.
const TEMP_MESSAGE_TTL: Duration = Duration::from_secs(16);

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

pub const SPOOL_THRESHOLD: usize = 10 * 1024 * 1024; // 10 MB

pub enum SpooledData {
    Memory(Vec<u8>),
    OnDisk(TempPath),
}

impl SpooledData {
    /// Reads a multipart field into memory (small) or a temp file (large),
    /// failing with `TooLarge` if more than `max_size` bytes are received.
    pub async fn from_multipart_field(
        mut field: axum::extract::multipart::Field<'_>,
        max_size: usize,
    ) -> Result<Self, FileError> {
        let mut data = Vec::with_capacity(8192);
        let mut on_disk: Option<(File, TempPath)> = None;
        let mut total: usize = 0;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| FileError::BadRequest(format!("failed to read file: {e}")))?
        {
            total += chunk.len();
            if total > max_size {
                return Err(FileError::TooLarge(format!(
                    "file exceeds {max_size} bytes"
                )));
            }

            if let Some((ref mut file, _)) = on_disk {
                file.write_all(&chunk).await?;
            } else {
                data.extend_from_slice(&chunk);
                if data.len() > SPOOL_THRESHOLD {
                    let (std_file, path) = NamedTempFile::new()?.into_parts();
                    let mut file = File::from_std(std_file);
                    file.write_all(&data).await?;
                    on_disk = Some((file, path));
                    data = Vec::new();
                }
            }
        }

        match on_disk {
            Some((mut file, path)) => {
                file.flush().await?;
                Ok(SpooledData::OnDisk(path))
            }
            None => Ok(SpooledData::Memory(data)),
        }
    }
}

/// Runs a Telegram request with a timeout, retrying according to policy.
///
/// - `RetryAfter` <= 5s: sleep and retry; longer: returned (mapped to 429).
/// - Network errors: retried with exponential backoff only if `retry_network` says so
///   (so non-idempotent requests that may have reached Telegram are not repeated).
/// - Timeouts are not retried and map to 504.
async fn retry_request<F, Fut, T>(
    timeout: Duration,
    what: &str,
    retry_network: fn(&reqwest::Error) -> bool,
    f: F,
) -> Result<T, FileError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, RequestError>>,
{
    let mut retries = 0;
    let mut delay = INITIAL_RETRY_DELAY;

    loop {
        let err = match tokio::time::timeout(timeout, f()).await {
            Ok(Ok(v)) => return Ok(v),
            Ok(Err(err)) => err,
            Err(_) => {
                return Err(FileError::Timeout(format!(
                    "Telegram API timeout on {what}"
                )))
            }
        };

        if retries >= MAX_RETRIES {
            return Err(err.into());
        }

        match &err {
            RequestError::RetryAfter(secs)
                if (secs.seconds() as u64) <= MAX_INTERNAL_RETRY_AFTER_SECS =>
            {
                tokio::time::sleep(Duration::from_secs(secs.seconds() as u64)).await;
            }
            RequestError::Network(e) if retry_network(e) => {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            _ => return Err(err.into()),
        }
        retries += 1;
    }
}

fn never_retry(_: &reqwest::Error) -> bool {
    false
}

fn only_connect(e: &reqwest::Error) -> bool {
    e.is_connect()
}

fn not_timeout(e: &reqwest::Error) -> bool {
    !e.is_timeout()
}

pub async fn upload_file(
    file: SpooledData,
    filename: String,
    caption: Option<String>,
    chat_id: i64,
) -> Result<UploadedFile, FileError> {
    let bot = ROUND_ROBIN_BOT.get_bot();

    // The temp file (if any) is kept alive by `file` until the API call completes.
    let (memory, path, _keep): (Option<Bytes>, Option<PathBuf>, Option<TempPath>) = match file {
        SpooledData::Memory(data) => (Some(Bytes::from(data)), None, None),
        SpooledData::OnDisk(p) => (None, Some(p.to_path_buf()), Some(p)),
    };

    let result = retry_request(
        Duration::from_secs(CONFIG.upload_timeout_secs),
        "upload",
        never_retry,
        || {
            let document = match (&memory, &path) {
                (Some(m), _) => InputFile::memory(m.clone()),
                (None, Some(p)) => InputFile::file(p.clone()),
                (None, None) => unreachable!(),
            }
            .file_name(filename.clone());
            let mut request = bot.send_document(ChatId(chat_id), document);
            request.caption = caption.clone();
            request.send()
        },
    )
    .await?;

    Ok(UploadedFile {
        backend: "bot".to_string(),
        data: MessageInfo {
            chat_id: result.chat.id.0,
            message_id: result.id.0,
        },
    })
}

/// Deletes the temporary forwarded message after its TTL (best effort, failures are logged).
fn schedule_temp_message_deletion(bot: Bot, message_id: MessageId) {
    tokio::spawn(async move {
        tokio::time::sleep(TEMP_MESSAGE_TTL).await;
        if let Err(err) = bot
            .delete_message(
                Recipient::Id(ChatId(CONFIG.telegram_temp_chat_id)),
                message_id,
            )
            .await
        {
            log::warn!(
                "Failed to delete temporary message {}: {}",
                message_id.0,
                err
            );
        }
    });
}

/// Returns the opened file and its size, or `None` if the message has no document.
pub async fn download_file(
    chat_id: i64,
    message_id: i32,
) -> Result<Option<(File, u64)>, FileError> {
    // file_id is bound to the bot that saw the message, so use one bot for the whole operation.
    let bot = ROUND_ROBIN_BOT.get_bot();

    let forwarded_message = retry_request(
        TELEGRAM_API_TIMEOUT,
        "forward_message",
        only_connect,
        || {
            bot.forward_message(
                ChatId(CONFIG.telegram_temp_chat_id),
                ChatId(chat_id),
                MessageId(message_id),
            )
            .send()
        },
    )
    .await?;

    schedule_temp_message_deletion(bot.clone(), forwarded_message.id);

    // If the message has no document, treat as no content
    let file_id = match forwarded_message.document() {
        Some(doc) => doc.file.id.clone(),
        None => return Ok(None),
    };

    let path = retry_request(TELEGRAM_API_TIMEOUT, "get_file", not_timeout, || {
        bot.get_file(file_id.clone()).send()
    })
    .await?
    .path;

    let file = File::open(path).await?;
    let len = file.metadata().await?.len();
    Ok(Some((file, len)))
}

#[derive(Debug, Default)]
pub struct CleanStats {
    pub removed: usize,
    pub errors: usize,
}

const FILE_MAX_AGE_SECS: u64 = 3600;

/// Removes old files from the local Bot API documents directories.
///
/// Directory names under the Bot API root contain bot tokens, so full paths
/// are never logged — only file names.
pub async fn clean_files() -> Result<CleanStats, FileError> {
    let bots_folder = "/var/lib/telegram-bot-api/";
    let documents_folder_name = "documents";
    let mut stats = CleanStats::default();

    let mut bots_folder = tokio::fs::read_dir(bots_folder).await?;

    while let Some(entry) = bots_folder.next_entry().await.transpose() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                log::error!("Failed to read bot directory entry: {err}");
                stats.errors += 1;
                continue;
            }
        };

        match entry.metadata().await {
            Ok(m) if m.is_dir() => {}
            Ok(_) => continue,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::error!("Failed to stat bot directory entry: {err}");
                    stats.errors += 1;
                }
                continue;
            }
        }

        let documents_folder_path = entry.path().join(documents_folder_name);
        let mut document_folder = match tokio::fs::read_dir(&documents_folder_path).await {
            Ok(v) => v,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::error!("Failed to read documents directory: {err}");
                    stats.errors += 1;
                }
                continue;
            }
        };

        while let Some(file) = document_folder.next_entry().await.transpose() {
            let file = match file {
                Ok(f) => f,
                Err(err) => {
                    log::error!("Failed to read documents directory entry: {err}");
                    stats.errors += 1;
                    continue;
                }
            };
            let name = file.file_name();

            let age_secs = match file.metadata().await {
                Ok(m) => m
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_secs()),
                Err(err) => {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        log::error!("Failed to stat file {name:?}: {err}");
                        stats.errors += 1;
                    }
                    continue;
                }
            };

            if age_secs.is_some_and(|age| age > FILE_MAX_AGE_SECS) {
                match tokio::fs::remove_file(file.path()).await {
                    Ok(_) => {
                        stats.removed += 1;
                        log::info!("File {name:?} removed");
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        stats.errors += 1;
                        log::error!("Failed to remove file {name:?}: {err}");
                    }
                }
            }
        }
    }

    Ok(stats)
}
