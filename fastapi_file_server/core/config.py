from typing import Optional

from pydantic import BaseModel
from pydantic_settings import (
    BaseSettings,
    SettingsConfigDict,
)


BotToken = str
TelethonSessionName = str


class TelethonConfig(BaseModel):
    APP_ID: int
    API_HASH: str


class EnvConfig(BaseSettings):
    model_config = SettingsConfigDict(env_file='.env', env_file_encoding='utf-8')

    API_KEY: str

    TELEGRAM_CHAT_ID: int

    BOT_TOKENS: Optional[list[BotToken]] = None

    TELETHON_APP_CONFIG: Optional[TelethonConfig] = None
    TELETHON_SESSIONS: Optional[list[TelethonSessionName]] = None

    SENTRY_DSN: str


env_config = EnvConfig()
