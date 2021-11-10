from datetime import datetime

from pydantic import BaseModel


class UploadedFile(BaseModel):
    id: int
    backend: str
    data: dict
    upload_time: datetime
