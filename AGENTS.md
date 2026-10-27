# AGENTS.md

## Project

Rust web server (Axum + teloxide) that acts as a file proxy: uploads files to Telegram chats via bot API, downloads them back on request. Single binary `telegram_files_server`, runs on port 8080.

## Commands

```bash
cargo fmt                  # format (required, lefthook enforced)
cargo clippy               # lint (required, lefthook + CI enforced)
cargo check                # typecheck (required, lefthook enforced)
cargo build --release      # production build
```

Unit tests: `cargo test` (error→HTTP status mapping, not-found classification).

## Environment

The app requires these env vars at startup (loaded via `dotenvy` from `.env` locally):

- `API_KEY` — auth key for API endpoints
- `API_URL` — Telegram Bot API base URL
- `TELEGRAM_CHAT_ID` — target chat for file uploads (i64)
- `TELEGRAM_AUDIO_CHAT_ID` — chat used for uploads with `file_type=audiobook` (i64)
- `TELEGRAM_TEMP_CHAT_ID` — temp chat used for download forwarding (i64)
- `BOT_TOKENS` — JSON array of bot token strings, e.g. `["token1","token2"]`
- `SENTRY_DSN` — Sentry project DSN

Optional (must be > 0 if set):

- `UPLOAD_TIMEOUT_SECS` — timeout of the Telegram upload call (default 600, → 504)
- `MAX_UPLOAD_SIZE` — max uploaded file size in bytes (default 2 GiB, → 413)
- `MAX_CONCURRENT_UPLOADS` — concurrent upload limit (default 4)
- `RUST_LOG` — log filter (default `info`)

`API_KEY` must be non-empty, `API_URL` a valid URL and `BOT_TOKENS` a non-empty JSON array; otherwise the app refuses to start.

In production, `scripts/env.sh` writes the environment variables to `.env` before starting the binary. Variables must already be set in the container's environment.

## Architecture

- `src/main.rs` — entrypoint: sets up Sentry, tracing, runs web server + cron concurrently via `tokio::join!`
- `src/config.rs` — once-cell lazy config from env vars
- `src/core/bot.rs` — round-robin bot pool across multiple bot tokens
- `src/core/views.rs` — Axum routes + auth middleware; body limit = MAX_UPLOAD_SIZE (+1 MiB)
- `src/core/file_utils.rs` — upload/download logic, forwarded temp messages are deleted by a spawned task ~16s after forward

Key routes: `POST /api/v1/files/upload/`, `GET /api/v1/files/download_by_message/{chat_id}/{message_id}`, `GET /health`, `GET /metrics`

A background cron (`clean_files`) every 5 minutes removes files older than 1 hour from `/var/lib/telegram-bot-api/documents/`.

## CI

- Push to `main`: runs clippy SARIF analysis + builds Docker image → pushes to GHCR + triggers deploy webhook
- PRs to `main`: runs clippy analysis only

## Gotchas

- `cargo clippy` must pass — it runs with `--all-features` in CI
- Lefthook hooks enforce `fmt`, `cargo check`, and `clippy`
- Production Docker image uses `linux/amd64` only