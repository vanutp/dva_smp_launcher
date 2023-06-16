import asyncio
from dataclasses import dataclass
from hashlib import sha1
from pathlib import Path

import httpx
from rich.progress import track, Progress

from build_cfg import SERVER_BASE
from src.config import get_minecraft_dir


def hash_file(path: Path) -> str:
    with open(path, 'rb') as f:
        return sha1(f.read()).hexdigest()


async def download_file(client: httpx.AsyncClient, url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    resp = await client.get(url)
    with open(path, 'wb') as f:
        f.write(resp.read())


@dataclass
class ModpackIndex:
    version: str
    asset_index: str
    main_class: str
    include: list[str]
    objects: dict[str, str]


async def sync_modpack() -> ModpackIndex:
    print('Проверка файлов сборки...', end='')
    index_resp = await httpx.AsyncClient().get(f'{SERVER_BASE}index.json')
    index_resp.raise_for_status()
    index = ModpackIndex(**index_resp.json())
    index.include = [Path(x) for x in index.include]
    mc_dir = get_minecraft_dir()

    to_hash = []
    for rel_include_path in index.include:
        include_path = mc_dir / rel_include_path
        if include_path.is_file():
            norm_rel_include_path = str(rel_include_path).replace('\\', '/')
            to_hash.append((norm_rel_include_path, include_path))
        elif include_path.is_dir():
            for obj_path in include_path.rglob('*'):
                if obj_path.is_dir():
                    continue
                rel_obj_path = obj_path.relative_to(mc_dir)
                norm_rel_obj_path = str(rel_obj_path).replace('\\', '/')
                to_hash.append((norm_rel_obj_path, obj_path))
    existing_objects = {}

    print('\r', end='')
    for obj, obj_path in track(to_hash, 'Проверка файлов сборки...'):
        existing_objects[obj] = hash_file(obj_path)

    for obj in existing_objects.keys():
        if obj not in index.objects:
            (mc_dir / obj).unlink()

    to_download = set()
    for obj, obj_hash in index.objects.items():
        if obj not in existing_objects or existing_objects[obj] != obj_hash:
            to_download.add(obj)

    async def download_coro():
        client = httpx.AsyncClient()
        while to_download:
            obj = to_download.pop()
            url = SERVER_BASE + obj
            await download_file(client, url, mc_dir / obj)

    async def report_progress(total: int):
        with Progress() as progress:
            t = progress.add_task('Загрузка файлов...', total=total)
            while to_download:
                current = total - len(to_download)
                progress.update(t, completed=current)
                await asyncio.sleep(0.5)
            progress.update(t, completed=total)

    if to_download:
        tasks = [report_progress(len(to_download))]
        for _ in range(8):
            tasks.append(download_coro())
        await asyncio.gather(*tasks)

    return index


__all__ = ['sync_modpack', 'ModpackIndex']
