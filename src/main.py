import asyncio

from src.config import load_config, save_config
from src.ely_by import authorize
from src.ely_by.utils import get_user
from src.utils.java import find_java
from src.utils.modpack import sync_modpack


async def _main():
    config = load_config()
    if not config.token:
        config.token = await authorize()
        save_config(config)
    if not config.java_path:
        config.java_path = find_java()
    user_info = await get_user(config.token)
    print(f'Вы вошли как {user_info.username}')
    await sync_modpack()


def main():
    asyncio.run(_main())
