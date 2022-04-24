from typing import Optional

from pydantic import BaseModel, BaseSettings


BotToken = str
TelethonSessionName = str


class TelethonConfig(BaseModel):
    APP_ID: int
    API_HASH: str


class EnvConfig(BaseSettings):
    API_KEY: str

    POSTGRES_USER: str
    POSTGRES_PASSWORD: str
    POSTGRES_HOST: str
    POSTGRES_PORT: int
    POSTGRES_DB: str

    TELEGRAM_CHAT_ID: int

    BOT_TOKENS: Optional[list[BotToken]]

    TELETHON_APP_CONFIG: Optional[TelethonConfig]
    TELETHON_SESSIONS: Optional[list[TelethonSessionName]]

    SENTRY_DSN: str

    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"


env_config = EnvConfig()
