from app.services.storages import StoragesContainer


async def on_start():
    await StoragesContainer.prepare()
