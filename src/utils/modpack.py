import asyncio
from dataclasses import dataclass, asdict
from hashlib import sha1
from pathlib import Path
from typing import Any
import json

import httpx
from rich.progress import track, Progress

from build_cfg import SERVER_BASE
from src.config import get_minecraft_dir, get_index_path, get_assets_dir, Config
from src.errors import LauncherError


def hash_file(path: Path) -> str:
    with open(path, 'rb') as f:
        return sha1(f.read()).hexdigest()


def get_files_in_dir(path: Path, rel_to: Path) -> list[str]:
    normalize = lambda x: str(x.relative_to(rel_to)).replace('\\', '/')
    if path.is_file():
        return [normalize(path)]
    files = []
    for file_path in path.rglob('*'):
        if file_path.is_dir():
            continue
        files.append(normalize(file_path))
    return files


async def download_file(client: httpx.AsyncClient, url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    resp = await client.get(url)
    resp.raise_for_status()
    with open(path, 'wb') as f:
        f.write(resp.read())


@dataclass
class ModpackIndex:
    modpack_name: str
    java_version: str
    minecraft_version: str
    modpack_version: str
    asset_index: str
    main_class: str
    libraries: list[dict]
    java_args: list[dict]
    game_args: list[dict]
    include: list[str]
    include_no_overwrite: list[str]
    objects: dict[str, str]
    client_filename: str


def indexes_from_data(data: Any) -> list[ModpackIndex]:
    return [ModpackIndex(**x) for x in data]


async def load_remote_indexes() -> list[ModpackIndex]:
    index_resp = await httpx.AsyncClient().get(f'{SERVER_BASE}/index.json')
    index_resp.raise_for_status()
    return indexes_from_data(index_resp.json())


def load_local_indexes(config: Config) -> list[ModpackIndex]:
    index_path = get_index_path(config)
    if not index_path.is_file():
        return []
    try:
        with open(index_path) as f:
            return indexes_from_data(json.load(f))
    except (json.JSONDecodeError, TypeError):
        return []


def save_local_index(config: Config, index: ModpackIndex) -> None:
    indexes = load_local_indexes(config)
    indexes = [x for x in indexes if x.modpack_name != index.modpack_name]
    indexes.append(index)
    with open(get_index_path(config), 'w') as f:
        json.dump([asdict(x) for x in indexes], f, indent=2)


async def get_modpack(config: Config, online: bool) -> ModpackIndex | None:
    indexes = await load_remote_indexes() if online else load_local_indexes(config)
    return next((x for x in indexes if x.modpack_name == config.modpack), None)


async def sync_modpack(
    config: Config, index: ModpackIndex, force_overwrite: bool = False
) -> None:
    print('Обновление сборки...', end='', flush=True)

    mc_dir = get_minecraft_dir(config, index.modpack_name)
    assets_dir = get_assets_dir(config)

    # [(is_asset, relative_path)]
    to_hash: list[tuple[bool, str]] = []
    for rel_include_path in index.include + (
        index.include_no_overwrite if force_overwrite else []
    ):
        include_path = mc_dir / Path(rel_include_path)
        to_hash.extend([(False, x) for x in get_files_in_dir(include_path, mc_dir)])
    to_hash.extend([(True, x) for x in get_files_in_dir(assets_dir, assets_dir)])

    existing_objects = {}
    print('\r', end='')
    for is_asset, obj in track(to_hash, 'Проверка файлов сборки...'):
        if is_asset:
            existing_objects['assets/' + obj] = hash_file(assets_dir / obj)
        else:
            existing_objects[obj] = hash_file(mc_dir / obj)

    for obj in existing_objects.keys():
        if obj.startswith('assets/'):
            continue
        if obj not in index.objects:
            (mc_dir / obj).unlink()

    existing_no_overwrite = set()
    if not force_overwrite:
        for rel_include_path in index.include_no_overwrite:
            include_path = mc_dir / Path(rel_include_path)
            existing_no_overwrite.update(get_files_in_dir(include_path, mc_dir))

    to_download = set()
    for obj, obj_hash in index.objects.items():
        if obj in existing_no_overwrite:
            continue
        if obj not in existing_objects or existing_objects[obj] != obj_hash:
            to_download.add(obj)

    async def download_coro():
        client = httpx.AsyncClient()
        while to_download:
            obj = to_download.pop()
            url = SERVER_BASE + '/' + index.modpack_name + '/' + obj
            if obj.startswith('assets/'):
                target_file = assets_dir / obj.removeprefix('assets/')
            else:
                target_file = mc_dir / obj
            max_retries = 6
            retries = 0
            while True:
                try:
                    await download_file(client, url, target_file)
                    break
                except httpx.TransportError as e:
                    retries += 1
                    if retries == max_retries:
                        raise LauncherError(
                            f'Не удалось загрузить модпак ({type(e).__name__})'
                        )
                    await asyncio.sleep(retries)

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

    save_local_index(config, index)
