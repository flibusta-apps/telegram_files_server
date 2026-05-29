#!/usr/bin/env sh

# Outputs all required env vars in .env format.
# Variables must already be set in the environment.

for var in API_KEY API_URL TELEGRAM_CHAT_ID TELEGRAM_TEMP_CHAT_ID BOT_TOKENS SENTRY_DSN; do
  eval "val=\$$var"
  if [ -n "$val" ]; then
    echo "${var}='${val}'"
  fi
done
