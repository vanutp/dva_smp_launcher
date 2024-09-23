import asyncio
from pathlib import Path
import hashlib
import aiofiles
import httpx
import logging


log_format = "%(name)s - %(levelname)s - %(message)s"
logging.basicConfig(level=logging.INFO, format=log_format)
logger = logging.getLogger(__name__)


async def hash_file(file_path: Path) -> str:
    hash_func = hashlib.sha1()
    async with aiofiles.open(file_path, "rb") as f:
        while True:
            chunk = await f.read(4096)
            if not chunk:
                break
            hash_func.update(chunk)
    return hash_func.hexdigest()


async def hash_files(file_paths: list[Path]) -> dict[Path, str]:
    tasks = [hash_file(file_path) for file_path in file_paths]
    hashes = await asyncio.gather(*tasks)
    return dict(zip(file_paths, hashes))


AUTHLIB_INJECTOR_URL = f"https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.5/authlib-injector-1.2.5.jar"


async def download_authlib_injector(work_dir: Path) -> Path:
    authlib_injector_path = work_dir / "authlib-injector.jar"
    if not authlib_injector_path.exists():
        async with httpx.AsyncClient() as client:
            logger.info(f"Downloading Authlib Injector from '{AUTHLIB_INJECTOR_URL}'")
            resp = await client.get(AUTHLIB_INJECTOR_URL, follow_redirects=True)
            resp.raise_for_status()
            async with aiofiles.open(authlib_injector_path, "wb") as f:
                await f.write(resp.content)

    return authlib_injector_path
