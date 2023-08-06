from typing import AsyncIterator, Optional

import telethon.client
import telethon.errors
import telethon.hints
import telethon.tl.types

from core.config import env_config


class BaseStorage:
    def __init__(self, channel_id: int, app_id: int, api_hash: str, session: str):
        self.channel_id = channel_id

        self.client = telethon.client.TelegramClient(session, app_id, api_hash)

        self.ready = False

    async def prepare(self):
        ...

    async def upload(
        self,
        file: telethon.hints.FileLike,
        file_size: int,
        caption: Optional[str] = None,
    ) -> Optional[tuple[int, int]]:
        try:
            uploaded_file = await self.client.upload_file(file, file_size=file_size)

            if caption:
                message = await self.client.send_file(
                    entity=self.channel_id, file=uploaded_file, caption=caption
                )
            else:
                message = await self.client.send_file(
                    entity=self.channel_id, file=uploaded_file
                )
        except telethon.errors.FilePartInvalidError:
            return None
        except telethon.errors.FilePartsInvalidError:
            return None
        except telethon.errors.PhotoInvalidError:
            return None

        if not message.media:
            return None

        return self.channel_id, message.id

    async def download(self, message_id: int) -> Optional[AsyncIterator[bytes]]:
        messages = await self.client.get_messages(self.channel_id, ids=[message_id])

        if not messages:
            return None

        message: Optional[telethon.tl.types.Message] = messages[0]

        if message is None:
            return None

        if message.media is None:
            return None

        return self.client.iter_download(message.media)


class UserStorage(BaseStorage):
    async def prepare(self):
        if self.ready:
            return

        await self.client.start()  # type: ignore

        if not await self.client.is_user_authorized():
            await self.client.sign_in()
            try:
                await self.client.sign_in(code=input("Enter code: "))
            except telethon.errors.SessionPasswordNeededError:
                await self.client.sign_in(password=input("Enter password: "))

        self.ready = True


class BotStorage(BaseStorage):
    def __init__(
        self,
        channel_id: int,
        app_id: int,
        api_hash: str,
        session: str,
        token: str,
    ) -> None:
        super().__init__(channel_id, app_id, api_hash, session)

        self.token = token

    async def prepare(self):
        if self.ready:
            return

        await self.client.start(bot_token=self.token)  # type: ignore

        self.ready = True


class StoragesContainer:
    BOT_STORAGES: list[BotStorage] = []
    USER_STORAGES: list[UserStorage] = []

    @classmethod
    async def prepare(cls):
        if not env_config.TELETHON_APP_CONFIG:
            return

        if env_config.BOT_TOKENS:
            cls.BOT_STORAGES: list[BotStorage] = [
                BotStorage(
                    env_config.TELEGRAM_CHAT_ID,
                    env_config.TELETHON_APP_CONFIG.APP_ID,
                    env_config.TELETHON_APP_CONFIG.API_HASH,
                    token.split(":")[0],
                    token,
                )
                for token in env_config.BOT_TOKENS
            ]

        if env_config.TELETHON_SESSIONS:
            cls.USER_STORAGES: list[UserStorage] = [
                UserStorage(
                    env_config.TELEGRAM_CHAT_ID,
                    env_config.TELETHON_APP_CONFIG.APP_ID,
                    env_config.TELETHON_APP_CONFIG.API_HASH,
                    session,
                )
                for session in env_config.TELETHON_SESSIONS
            ]

        for storage in [*cls.BOT_STORAGES, *cls.USER_STORAGES]:
            await storage.prepare()
