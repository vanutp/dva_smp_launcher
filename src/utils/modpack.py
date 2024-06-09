import asyncio
from dataclasses import dataclass
from hashlib import sha1
from pathlib import Path

import httpx
import rich
from rich.progress import track, Progress

from build_cfg import SERVER_BASE
from src.config import get_minecraft_dir, Config, get_dirs
from src.errors import LauncherError


def hash_file(path: Path) -> str:
    with open(path, 'rb') as f:
        return sha1(f.read()).hexdigest()


async def download_file(client: httpx.AsyncClient, url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    resp = await client.get(url)
    resp.raise_for_status()
    with open(path, 'wb') as f:
        f.write(resp.read())


@dataclass
class ModpackIndex:
    modpack_name: str
    version: str
    asset_index: str
    main_class: str
    classpath: list[str] | None
    java_args: list[dict]
    game_args: list[dict]
    include: list[str]
    objects: dict[str, str]
    client_filename: str


def get_assets_dir(config: Config):
    return Path(config.assets_dir or (get_dirs().user_data_path / 'assets'))


async def load_indexes() -> list[ModpackIndex]:
    index_resp = await httpx.AsyncClient().get(f'{SERVER_BASE}index.json')
    index_resp.raise_for_status()
    indexes = [ModpackIndex(**x) for x in index_resp.json()]
    for index in indexes:
        index.include = [Path(x) for x in index.include]
    return indexes


class ModpackNotFoundError(Exception):
    pass


async def sync_modpack(config: Config) -> ModpackIndex:
    print('Проверка файлов сборки...', end='', flush=True)
    indexes = await load_indexes()
    indexes = [x for x in indexes if x.modpack_name == config.modpack]
    if not indexes:
        print()
        rich.print(
            '[red]Ошибка! Сборка не найдена на сервере. Нажмите Enter чтобы выбрать сборку[/red]'
        )
        input()
        raise ModpackNotFoundError()
    index = indexes[0]

    mc_dir = get_minecraft_dir(index.modpack_name)
    assets_dir = get_assets_dir(config)

    # [(is_asset, relative_path)]
    to_hash: list[tuple[bool, str]] = []
    for rel_include_path in index.include:
        include_path = mc_dir / rel_include_path
        if include_path.is_file():
            to_hash.append((False, str(rel_include_path)))
        elif include_path.is_dir():
            for obj_path in include_path.rglob('*'):
                if obj_path.is_dir():
                    continue
                rel_obj_path = obj_path.relative_to(mc_dir)
                norm_rel_obj_path = str(rel_obj_path).replace('\\', '/')
                to_hash.append((False, norm_rel_obj_path))
    for obj_path in assets_dir.rglob('*'):
        if obj_path.is_dir():
            continue
        rel_obj_path = obj_path.relative_to(assets_dir)
        norm_rel_obj_path = str(rel_obj_path).replace('\\', '/')
        to_hash.append((True, norm_rel_obj_path))

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

    to_download = set()
    for obj, obj_hash in index.objects.items():
        if obj not in existing_objects or existing_objects[obj] != obj_hash:
            to_download.add(obj)

    async def download_coro():
        client = httpx.AsyncClient()
        while to_download:
            obj = to_download.pop()
            url = SERVER_BASE + index.modpack_name + '/' + obj
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

    return index
