from typing import Any, Optional

from fastapi import UploadFile

from app.models import UploadBackends, UploadedFile
from app.services.storages import BotStorage, StoragesContainer, UserStorage


class Wrapper:
    def __init__(self, wrapped: Any, filename: str) -> None:
        self.wrapped = wrapped
        self.filename = filename

    def seekable(self):
        return True

    def __getattr__(self, __name: str) -> Any:
        if __name == "name":
            return self.filename

        return getattr(self.wrapped, __name)


class FileUploader:
    _bot_storage_index = 0
    _user_storage_index = 0

    @classmethod
    @property
    def bot_storages(cls) -> list[BotStorage]:
        return StoragesContainer.BOT_STORAGES

    @classmethod
    @property
    def user_storages(cls) -> list[UserStorage]:
        return StoragesContainer.USER_STORAGES

    def __init__(self, file: UploadFile, caption: Optional[str] = None) -> None:
        self.file = file
        self.caption = caption

        self.upload_data: Optional[dict] = None
        self.upload_backend: Optional[UploadBackends] = None

    async def _upload(self) -> bool:
        if not self.bot_storages and not self.user_storages:
            raise ValueError("Files storage not exist!")

        if await self._upload_via(UploadBackends.bot):
            return True

        return await self._upload_via(UploadBackends.user)

    async def _upload_via(self, storage_type: UploadBackends) -> bool:
        if storage_type == UploadBackends.bot:
            storage = self.get_bot_storage()
        else:
            storage = self.get_user_storage()

        wrapped = Wrapper(self.file.file, self.file.filename)

        data = await storage.upload(wrapped, caption=self.caption)

        if not data:
            return False

        self.upload_data = {"chat_id": data[0], "message_id": data[1]}
        self.upload_backend = storage_type

        return True

    async def _save_to_db(self) -> UploadedFile:
        return await UploadedFile.objects.create(
            backend=self.upload_backend,
            data=self.upload_data,
        )

    @classmethod
    def get_bot_storage(cls) -> BotStorage:
        if not cls.bot_storages:
            raise ValueError("Aiogram storage not exist!")

        bot_storages: list[BotStorage] = cls.bot_storages  # type: ignore

        cls._bot_storage_index = (cls._bot_storage_index + 1) % len(bot_storages)

        return bot_storages[cls._bot_storage_index]

    @classmethod
    def get_user_storage(cls) -> UserStorage:
        if not cls.user_storages:
            raise ValueError("Telethon storage not exists!")

        user_storages: list[UserStorage] = cls.user_storages  # type: ignore

        cls._user_storage_index = (cls._user_storage_index + 1) % len(user_storages)

        return user_storages[cls._user_storage_index]

    @classmethod
    async def upload(
        cls, file: UploadFile, caption: Optional[str] = None
    ) -> Optional[UploadedFile]:
        uploader = cls(file, caption)
        upload_result = await uploader._upload()

        if not upload_result:
            return None

        return await uploader._save_to_db()
