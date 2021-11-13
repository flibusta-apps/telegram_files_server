from datetime import datetime

from pydantic import BaseModel, constr


class CreateUploadedFile(BaseModel):
    backend: constr(max_length=16)  # type: ignore
    data: dict
    upload_time: datetime


class UploadedFile(BaseModel):
    id: int
    backend: str
    data: dict
    upload_time: datetime
