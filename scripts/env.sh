#!/usr/bin/env sh

# Outputs all required env vars in .env format.
# Variables must already be set in the environment.

for var in API_KEY API_URL TELEGRAM_CHAT_ID TELEGRAM_AUDIO_CHAT_ID TELEGRAM_TEMP_CHAT_ID BOT_TOKENS SENTRY_DSN UPLOAD_TIMEOUT_SECS MAX_UPLOAD_SIZE MAX_CONCURRENT_UPLOADS; do
  eval "val=\$$var"
  if [ -n "$val" ]; then
    echo "${var}='${val}'"
  fi
done
