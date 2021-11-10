from fastapi import File, UploadFile
from starlette import status
from fastapi import APIRouter, HTTPException

from app.serializers import UploadedFile
from app import models
from app.services.file_uploader import FileUploader


router = APIRouter(
    prefix="/api/v1",
)


@router.get("/files", response_model=list[UploadedFile])
async def get_files():
    return await models.UploadedFile.objects.all()


@router.get("/files/{file_id}", response_model=UploadedFile, responses={
    404: {},
})
async def get_file(file_id: int):
    uploaded_file = await models.UploadedFile.objects.get_or_none(id=file_id)

    if not uploaded_file:
        raise HTTPException(status.HTTP_404_NOT_FOUND)

    return uploaded_file


@router.post("/files", response_model=UploadedFile)
async def upload_file(file: UploadFile = File({})):
    return await FileUploader.upload(file)


@router.delete("/files/{file_id}", response_model=UploadedFile, responses={
    400: {}
})
async def delete_file(file_id: int):
    uploaded_file = await models.UploadedFile.objects.get_or_none(id=file_id)

    if not uploaded_file:
        raise HTTPException(status.HTTP_400_BAD_REQUEST)

    await uploaded_file.delete()
    return uploaded_file
