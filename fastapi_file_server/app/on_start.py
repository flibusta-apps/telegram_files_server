from app.services.file_uploader import FileUploader


async def on_start():
    await FileUploader.prepare()
