import asyncio
import json
import shutil
import subprocess
from argparse import ArgumentParser
from distutils.dir_util import copy_tree
from hashlib import sha1
from pathlib import Path

import httpx
from pydantic import BaseModel
from tqdm import tqdm


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


def get_new_asset_hashes(asset_index: dict) -> dict[Path, str]:
    res = {}
    for obj in asset_index['objects'].values():
        obj_hash = obj['hash']
        res[Path(obj_hash[:2], obj_hash)] = obj_hash
    return res


def exec_custom_cmd(cmd: list[str] | str | None):
    if cmd:
        subprocess.check_call(cmd, shell=True)

class ModpackSpec(BaseModel):
    exec_before: str | None = None
    exec_after: str | None = None
    version_data_path: Path
    instance_dir: Path
    copy_extra: list[str]
    modpack_name: str
    clean_forge_libs_path: Path | None
    forge_libs_list: list[str] | None


class ModpackIndex(BaseModel):
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


class ModpackGenerator:
    spec: ModpackSpec
    target_dir: Path
    version_data: dict

    def __init__(self, spec: ModpackSpec):
        self.spec = spec
        self.target_dir = Path('modpacks') / spec.modpack_name
        self.target_dir.mkdir(parents=True, exist_ok=True)
        with open(spec.version_data_path) as f:
            self.version_data = json.load(f)

    def get_client_filename(self):
        version = self.version_data['jar']
        return f'{version}.jar'

    def get_new_lib_hashes(self) -> dict[Path, tuple[str | None, str]]:
        res = {}
        for lib in self.version_data['libraries']:
            lib_path = lib_name_to_path(lib['name'])
            if 'downloads' in lib:
                res[lib_path] = (
                    lib['downloads']['artifact']['sha1'],
                    lib['downloads']['artifact']['url'],
                )
            else:
                res[lib_path] = (None, lib['url'] + str(lib_path))
        return res

    async def download_missing_libs(self):
        libs_dir = self.target_dir / 'libraries'
        existing = hash_dir(libs_dir)
        new = self.get_new_lib_hashes()
        if not self.spec.clean_forge_libs_path:
            for lib in existing:
                if lib not in new:
                    (libs_dir / lib).unlink()
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

    async def download_missing_assets(self) -> None:
        asset_config = self.version_data['assetIndex']
        assets_dir = self.target_dir / 'assets'
        asset_index_path = assets_dir / 'indexes' / (asset_config['id'] + '.json')
        if asset_index_path.is_file():
            with open(asset_index_path, 'rb') as f:
                index_hash = sha1(f.read()).hexdigest()
        else:
            index_hash = None
        if index_hash != asset_config['sha1']:
            await download_file(
                httpx.AsyncClient(), asset_config['url'], asset_index_path
            )
        with open(asset_index_path) as f:
            asset_index = json.load(f)

        print('Hashing existing assets...')
        existing = hash_dir(assets_dir / 'objects')
        new = get_new_asset_hashes(asset_index)
        for obj in existing:
            if obj not in new:
                (assets_dir / 'objects' / obj).unlink()
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

    async def download_client(self) -> None:
        client_path = self.target_dir / self.get_client_filename()
        if client_path.is_file():
            with open(client_path, 'rb') as f:
                client_hash = sha1(f.read()).hexdigest()
        else:
            client_hash = None
        client_info = self.version_data['downloads']['client']
        if client_hash != client_info['sha1']:
            print('Downloading client...')
            await download_file(httpx.AsyncClient(), client_info['url'], client_path)

    def copy_mods(self):
        mods_target = self.target_dir / 'mods'
        mods_target.mkdir(exist_ok=True)
        exising = hash_dir(mods_target)
        print('Copying mods...')
        for mod in exising:
            if not (self.spec.instance_dir / 'mods' / mod.name).is_file():
                (mods_target / mod).unlink()
        for mod in (self.spec.instance_dir / 'mods').glob('*.jar'):
            shutil.copy2(mod, mods_target / mod.name)

    def copy_forge_libs(self):
        if not self.spec.clean_forge_libs_path:
            return
        print('Copying forge libs...')
        source_path = Path(self.spec.clean_forge_libs_path)
        target_path = self.target_dir / 'libraries'
        target_path.parent.mkdir(parents=True, exist_ok=True)
        if source_path.exists():
            copy_tree(str(source_path), str(target_path))
        else:
            raise FileNotFoundError(f'Forge libraries dir not found')

    def copy_extra(self):
        print('Copying extra data (configs, etc.)...')
        for obj in self.spec.copy_extra:
            source_path = self.spec.instance_dir / obj
            target_path = self.target_dir / obj
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

    def create_index(self) -> ModpackIndex:
        print('Creating index file...')
        hashes = {
            str(k): v
            for k, v in hash_dir(self.target_dir, exclude=[Path('index.json')]).items()
        }
        return ModpackIndex(
            modpack_name=self.spec.modpack_name,
            version=self.version_data['jar'],
            asset_index=self.version_data['assetIndex']['id'],
            main_class=self.version_data['mainClass'],
            classpath=self.spec.forge_libs_list,
            java_args=self.version_data['arguments']['jvm'],
            game_args=self.version_data['arguments']['game'],
            include=[
                'libraries',
                'mods',
                self.get_client_filename(),
                *self.spec.copy_extra,
            ],
            objects=hashes,
            client_filename=self.get_client_filename(),
        )

    async def generate(self) -> ModpackIndex:
        exec_custom_cmd(self.spec.exec_before)
        self.copy_forge_libs()
        await self.download_missing_libs()
        await self.download_missing_assets()
        await self.download_client()
        self.copy_mods()
        self.copy_extra()
        index = self.create_index()
        exec_custom_cmd(self.spec.exec_after)
        return index


class Spec(BaseModel):
    exec_before_all: str | None = None
    exec_after_all: str | None = None
    modpacks: list[ModpackSpec]


async def main():
    parser = ArgumentParser()
    parser.add_argument('--only')
    args = parser.parse_args()
    index_path = Path('modpacks') / 'index.json'

    with open('spec.json') as f:
        spec = Spec.model_validate_json(f.read())
    exec_custom_cmd(spec.exec_before_all)

    indexes: dict[str, dict] = {}
    if index_path.exists():
        indexes = {x['modpack_name']: x for x in json.loads(index_path.read_text())}

    for modpack in spec.modpacks:
        if args.only and modpack.modpack_name != args.only:
            continue
        print(f'Generating {modpack.modpack_name}')
        indexes[modpack.modpack_name] = (await ModpackGenerator(modpack).generate()).model_dump(mode='json')
        print('Done')

    with open(index_path, 'w') as f:
        json.dump(list(indexes.values()), f)

    exec_custom_cmd(spec.exec_after_all)


asyncio.run(main())
