from typing import Optional

from fastapi import UploadFile

from app.serializers import Data, UploadBackend, UploadedFile
from app.services.storages import BotStorage, StoragesContainer, UserStorage


def seekable(*args, **kwargs):
    return False


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

    def __init__(
        self, file: UploadFile, file_size: int, caption: Optional[str] = None
    ) -> None:
        self.file = file
        self.file_size = file_size
        self.caption = caption

        self.upload_data: Optional[Data] = None
        self.upload_backend: Optional[UploadBackend] = None

    async def _upload(self) -> bool:
        if not self.bot_storages and not self.user_storages:
            raise ValueError("Files storage not exist!")

        if await self._upload_via(UploadBackend.bot):
            return True

        return await self._upload_via(UploadBackend.user)

    async def _upload_via(self, storage_type: UploadBackend) -> bool:
        if storage_type == UploadBackend.bot:
            storage = self.get_bot_storage()
        else:
            storage = self.get_user_storage()

        assert self.file.filename

        setattr(self.file, "seekable", seekable)  # noqa: B010

        data = await storage.upload(
            self.file,  # type: ignore
            file_size=self.file_size,
            filename=self.file.filename,
            caption=self.caption,
        )

        if not data:
            return False

        self.upload_data = {"chat_id": data[0], "message_id": data[1]}
        self.upload_backend = storage_type

        return True

    def get_result(self) -> UploadedFile:
        assert self.upload_backend is not None
        assert self.upload_data is not None

        return UploadedFile(backend=self.upload_backend, data=self.upload_data)

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
        cls, file: UploadFile, file_size: int, caption: Optional[str] = None
    ) -> Optional[UploadedFile]:
        uploader = cls(file, file_size, caption)
        upload_result = await uploader._upload()

        if not upload_result:
            return None

        return uploader.get_result()
