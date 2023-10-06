import asyncio
import json
import shutil
from distutils.dir_util import copy_tree
from hashlib import sha1
from pathlib import Path

import httpx
from tqdm import tqdm

version_data_path = '/home/fox/.minecraft/versions/quilt-loader-0.21.0-1.20.1/quilt-loader-0.21.0-1.20.1.json'
instance_dir = '/home/fox/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/VanillaPotato/.minecraft'
cfg_copy_extra = [
    'authlib-injector.jar',
    'servers.dat',
    'config',
]
modpack_name = 'Vanilla_Potato'
clean_forge_libs_path = ''
forge_is_a_piece_of_crap = []

target_dir = Path('modpacks') / modpack_name
target_dir.mkdir(parents=True, exist_ok=True)
libs_dir = target_dir / 'libraries'
assets_dir = target_dir / 'assets'

with open(version_data_path) as f:
    version_data = json.load(f)


async def download_file(client: httpx.AsyncClient, url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    resp = await client.get(url)
    with open(path, 'wb') as f:
        f.write(resp.read())


def hash_dir(directory: Path, exclude: list[Path] = None) -> dict[Path, str]:
    if exclude is None:
        exclude = []
    res = {}
    for path in directory.rglob('*'):
        if path.is_dir():
            continue
        relpath = path.relative_to(directory)
        if relpath in exclude:
            continue
        with open(path, 'rb') as f:
            res[relpath] = sha1(f.read()).hexdigest()
    return res


def lib_name_to_path(lib_name: str) -> Path:
    parts = lib_name.split(':')
    assert len(parts) in [3, 4]
    res = Path(*parts[0].split('.'), parts[1], parts[2])
    if len(parts) == 4:
        res = res / (parts[1] + '-' + parts[2] + '-' + parts[3] + '.jar')
    else:
        res = res / (parts[1] + '-' + parts[2] + '.jar')
    return res


def get_new_lib_hashes() -> dict[Path, tuple[str | None, str]]:
    res = {}
    for lib in version_data['libraries']:
        lib_path = lib_name_to_path(lib['name'])
        if 'downloads' in lib:
            res[lib_path] = (
                lib['downloads']['artifact']['sha1'],
                lib['downloads']['artifact']['url'],
            )
        else:
            res[lib_path] = (None, lib['url'] + str(lib_path))
    return res


async def download_missing_libs():
    existing = hash_dir(libs_dir)
    new = get_new_lib_hashes()
    # for lib in existing:
    #     if lib not in new:
    #         (libs_dir / lib).unlink()
    to_download = []
    for lib in new:
        if lib not in existing or (
            existing[lib]
            != new[lib][0]
            # and new[lib][0] is not None
        ):
            to_download.append(lib)

    if to_download:
        print('Downloading missing libraries...')
        client = httpx.AsyncClient()
        for lib in tqdm(to_download):
            await download_file(client, new[lib][1], libs_dir / lib)


def get_new_asset_hashes(asset_index: dict) -> dict[Path, str]:
    res = {}
    for obj in asset_index['objects'].values():
        obj_hash = obj['hash']
        res[Path(obj_hash[:2], obj_hash)] = obj_hash
    return res


async def download_missing_assets() -> None:
    asset_config = version_data['assetIndex']
    asset_index_path = assets_dir / 'indexes' / (asset_config['id'] + '.json')
    if asset_index_path.is_file():
        with open(asset_index_path, 'rb') as f:
            index_hash = sha1(f.read()).hexdigest()
    else:
        index_hash = None
    if index_hash != asset_config['sha1']:
        await download_file(httpx.AsyncClient(), asset_config['url'], asset_index_path)
    with open(asset_index_path) as f:
        asset_index = json.load(f)

    print('Hashing existing assets...')
    existing = hash_dir(assets_dir / 'objects')
    new = get_new_asset_hashes(asset_index)
    for obj in existing:
        if obj not in new:
            (assets_dir / obj).unlink()
    to_download = set()
    for obj in new:
        if obj not in existing or existing[obj] != new[obj]:
            to_download.add(obj)

    async def download_coro():
        client = httpx.AsyncClient()
        while to_download:
            obj = to_download.pop()
            url = 'https://resources.download.minecraft.net/' + str(obj)
            await download_file(client, url, assets_dir / 'objects' / obj)

    async def report_progress(total: int):
        t = tqdm(total=total)
        while to_download:
            current = total - len(to_download)
            t.update(current - t.n)
            await asyncio.sleep(0.5)
        t.update(total - t.n)
        t.close()

    if to_download:
        print('Downloading missing assets...')
        tasks = [report_progress(len(to_download))]
        for _ in range(8):
            tasks.append(download_coro())
        await asyncio.gather(*tasks)


async def download_client() -> None:
    client_path = target_dir / 'client.jar'
    if client_path.is_file():
        with open(client_path, 'rb') as f:
            client_hash = sha1(f.read()).hexdigest()
    else:
        client_hash = None
    client_info = version_data['downloads']['client']
    if client_hash != client_info['sha1']:
        print('Downloading client...')
        await download_file(httpx.AsyncClient(), client_info['url'], client_path)


def copy_mods():
    mods_target = target_dir / 'mods'
    mods_target.mkdir(exist_ok=True)
    exising = hash_dir(mods_target)
    print('Copying mods...')
    for mod in exising:
        if not Path(instance_dir, 'mods', mod.name).is_file():
            (mods_target / mod).unlink()
    for mod in Path(instance_dir, 'mods').glob('*.jar'):
        shutil.copy2(mod, mods_target / mod.name)


def copy_forge_libs():
    if not clean_forge_libs_path:
        return
    print('Copying forge libs...')
    source_path = Path(clean_forge_libs_path)
    target_path = target_dir / 'libraries'
    target_path.parent.mkdir(parents=True, exist_ok=True)
    if source_path.exists():
        copy_tree(str(source_path), str(target_path))
    else:
        raise FileNotFoundError(f'Forge libraries dir not found')


def copy_extra():
    print('Copying extra data (configs, etc.)...')
    for obj in cfg_copy_extra:
        source_path = Path(instance_dir) / obj
        target_path = target_dir / obj
        target_path.parent.mkdir(parents=True, exist_ok=True)
        if target_path.is_dir():
            shutil.rmtree(target_path)
        elif target_path.is_file():
            target_path.unlink()
        if source_path.is_dir():
            shutil.copytree(source_path, target_path)
        elif source_path.is_file():
            shutil.copy2(source_path, target_path)
        else:
            raise FileNotFoundError(f'Extra file/directory {obj} not found')


def create_index() -> None:
    print('Creating index file...')
    hashes = {
        str(k): v for k, v in hash_dir(target_dir, exclude=[Path('index.json')]).items()
    }
    index = {
        'version': version_data['jar'],
        'asset_index': version_data['assetIndex']['id'],
        'main_class': version_data['mainClass'],
        'classpath': forge_is_a_piece_of_crap,
        'java_args': version_data['arguments']['jvm'],
        'game_args': version_data['arguments']['game'],
        'include': [
            'libraries',
            'mods',
            'client.jar',
            *cfg_copy_extra,
        ],
        'objects': hashes,
    }
    with open(target_dir / 'index.json', 'w') as f:
        json.dump(index, f)


async def main():
    copy_forge_libs()
    await download_missing_libs()
    await download_missing_assets()
    await download_client()
    copy_mods()
    copy_extra()
    create_index()
    print('Done!')


asyncio.run(main())
