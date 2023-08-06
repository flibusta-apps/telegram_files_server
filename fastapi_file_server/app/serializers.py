import enum

from pydantic import BaseModel
from typing_extensions import TypedDict


class UploadBackend(enum.StrEnum):
    bot = "bot"
    user = "user"


class Data(TypedDict):
    chat_id: int
    message_id: int


class UploadedFile(BaseModel):
    backend: UploadBackend
    data: Data
