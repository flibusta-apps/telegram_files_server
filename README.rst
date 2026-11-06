Telegram files server
=====================

An Axum file proxy backed by a local Telegram Bot API server. Listens on port 8080.

Deployment requirements
-----------------------

``API_URL`` must point to a Telegram Bot API server running with ``--local``.
Cloud ``api.telegram.org`` URLs are rejected at startup: downloads open the
``getFile.file_path`` directly, rather than downloading it over HTTP. Mount the
Bot API data volume into this service at the same absolute path (normally
``/var/lib/telegram-bot-api``).

The production image runs as UID/GID ``101:101`` (same as the ``telegram-bot-api`` user
of the aiogram/telegram-bot-api image, which owns the shared volume). Shared files must be
readable and their parent directories traversable by UID 101. The cleanup
job also requires write permission on the Bot API documents directories.
Temporary upload spools use the container's writable temporary directory.

Set ``API_KEY``, ``API_URL``, ``TELEGRAM_CHAT_ID``, ``TELEGRAM_AUDIO_CHAT_ID``,
``TELEGRAM_TEMP_CHAT_ID``, ``BOT_TOKENS`` (a non-empty JSON array of token strings),
and ``SENTRY_DSN`` in the process/container environment. The startup script
executes the binary directly and never writes secrets to a file. A local
``.env`` may optionally be loaded for development; it is not required.

Limits and timeouts
-------------------

All numeric settings below must be greater than zero when set:

* ``MAX_UPLOAD_SIZE``: maximum file size in bytes (default 2 GiB).
* ``MAX_CONCURRENT_UPLOADS``: concurrent receiving/upload operations (default 4).
* ``UPLOAD_PERMIT_TIMEOUT_SECS``: maximum wait for an upload slot (default 60;
  HTTP 503 on timeout).
* ``UPLOAD_BODY_IDLE_TIMEOUT_SECS``: incoming body idle timeout, reset by
  non-empty body data, including multipart headers and unknown fields (default
  60; HTTP 408 on timeout). This is not a total upload deadline.
* ``UPLOAD_TIMEOUT_SECS``: Telegram upload call timeout (default 600; HTTP 504).
* ``GET_FILE_TIMEOUT_SECS``: Telegram getFile call timeout (default 600; HTTP 504).

If cleanup removes a cached file between getFile and opening it, the server
repeats getFile and open once. Other filesystem failures are not retried.

Authentication
--------------

Upload, download and ``/metrics`` require ``Authorization: Bearer <API_KEY>``
or the legacy raw ``Authorization: <API_KEY>`` header. Configure monitoring
scrapers accordingly. ``/health`` remains unauthenticated.
