"""
Most of the paths are from https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaUtils.cpp
"""

import os.path
import platform
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryFile
from urllib.parse import urlencode
from zipfile import ZipFile

import httpx
import inquirer.errors
import rich
from rich.progress import Progress, track

from src.compat import ismac, islinux, iswin
from src.config import Config, get_data_dir
from src import tui

if iswin():
    from winreg import (
        OpenKeyEx,
        HKEY_LOCAL_MACHINE,
        KEY_READ,
        KEY_ENUMERATE_SUB_KEYS,
        EnumKey,
        CloseKey,
        QueryValueEx,
    )


@dataclass
class JavaInstall:
    version: str
    path: str


JAVA_VERSION_RGX = re.compile(r'"(.*)?"')


def check_java(path: JavaInstall | str | Path) -> JavaInstall | None:
    if isinstance(path, JavaInstall):
        path = path.path
    elif isinstance(path, Path):
        path = str(path)
    path = shutil.which(path)
    if not path or not os.path.isfile(path):
        return None
    try:
        version_result = subprocess.check_output(
            [path, '-version'], stderr=subprocess.STDOUT
        ).decode()
    except subprocess.CalledProcessError:
        return None
    match = JAVA_VERSION_RGX.search(version_result)
    if not match:
        return None
    version = match.group(1)
    return JavaInstall(
        path=path,
        version=version,
    )


def is_good_version(required_version: str, java: JavaInstall) -> bool:
    return java.version == required_version or java.version.startswith(
        f'{required_version}.'
    )


def find_java_in_registry(
    key_name: str, subkey_suffix: str, java_dir_key: str
) -> list[JavaInstall | None]:
    try:
        key = OpenKeyEx(
            HKEY_LOCAL_MACHINE, key_name, access=KEY_READ | KEY_ENUMERATE_SUB_KEYS
        )
    except OSError:
        return []

    subkeys = []
    i = 0
    while True:
        try:
            subkeys.append(EnumKey(key, i))
            i += 1
        except OSError:
            break

    CloseKey(key)

    res = []
    for subkey in subkeys:
        key_path = key_name + '\\' + subkey + subkey_suffix
        key = OpenKeyEx(HKEY_LOCAL_MACHINE, key_path)
        try:
            java_dir_value = QueryValueEx(key, java_dir_key)[0]
        except OSError:
            pass
        else:
            exe_path = os.path.join(java_dir_value, 'bin', 'java.exe')
            res.append(JavaInstall(version=subkey, path=exe_path))
        finally:
            CloseKey(key)

    return res


def find_java_win() -> list[JavaInstall]:
    res = []
    res.extend(
        find_java_in_registry(r'SOFTWARE\Eclipse Adoptium\JDK', r'\hotspot\MSI', 'Path')
    )
    res.extend(
        find_java_in_registry(r'SOFTWARE\Eclipse Adoptium\JRE', r'\hotspot\MSI', 'Path')
    )
    res.extend(
        find_java_in_registry(r'SOFTWARE\AdoptOpenJDK\JDK', r'\hotspot\MSI', 'Path')
    )
    res.extend(
        find_java_in_registry(r'SOFTWARE\AdoptOpenJDK\JRE', r'\hotspot\MSI', 'Path')
    )
    res.extend(
        find_java_in_registry(
            r'SOFTWARE\Eclipse Foundation\JDK', r'\hotspot\MSI', 'Path'
        )
    )
    res.extend(
        find_java_in_registry(
            r'SOFTWARE\Eclipse Foundation\JRE', r'\hotspot\MSI', 'Path'
        )
    )
    res.extend(find_java_in_registry(r'SOFTWARE\JavaSoft\JDK', '', 'JavaHome'))
    res.extend(find_java_in_registry(r'SOFTWARE\JavaSoft\JRE', '', 'JavaHome'))
    res.extend(
        find_java_in_registry(r'SOFTWARE\Microsoft\JDK', r'\hotspot\MSI', 'Path')
    )
    res.extend(
        find_java_in_registry(r'SOFTWARE\Azul Systems\Zulu', r'', 'InstallationPath')
    )
    res.extend(
        find_java_in_registry(r'SOFTWARE\BellSoft\Liberica', r'', 'InstallationPath')
    )
    return res


def find_java_in_dir(
    dir_: str, *, suffix: str = '', startswith: str = ''
) -> list[JavaInstall | None]:
    suffix = Path(suffix)
    res = []
    for subdir in Path(dir_).glob('*'):
        if subdir.is_file():
            continue
        if startswith and not subdir.name.startswith(startswith):
            continue
        res.append(check_java(subdir / suffix / 'bin' / 'java'))
    return res


def find_java_linux() -> list[JavaInstall | None]:
    res = []
    res.extend(find_java_in_dir('/usr/java'))
    res.extend(find_java_in_dir('/usr/lib/jvm'))
    res.extend(find_java_in_dir('/usr/lib64/jvm'))
    res.extend(find_java_in_dir('/usr/lib32/jvm'))
    res.extend(find_java_in_dir('/opt/jdk'))
    return res


def find_java_macos() -> list[JavaInstall | None]:
    res = []
    res.extend(
        find_java_in_dir('/Library/Java/JavaVirtualMachines', suffix='Contents/Home')
    )
    res.extend(
        find_java_in_dir(
            '/System/Library/Java/JavaVirtualMachines', suffix='Contents/Home'
        )
    )
    res.extend(find_java_in_dir('/usr/local/opt', startswith='openjdk'))
    res.extend(find_java_in_dir('/opt/homebrew/opt', startswith='openjdk'))
    return res


def validate_user_java(required_version: str, path: str):
    java = check_java(path)
    if not java:
        raise inquirer.errors.ValidationError(
            path, reason='Java не найдена по этому пути'
        )
    if not is_good_version(required_version, java):
        raise inquirer.errors.ValidationError(
            path, reason=f'Неправильная версия Java, нужна {required_version}'
        )
    return True


def fix_java_path(path: str) -> str:
    if path.endswith('javaw.exe'):
        return path.removesuffix('javaw.exe') + 'java.exe'
    else:
        return path


def ask_user_java(required_version: str, default: str = None) -> JavaInstall | None:
    java_filename = 'java.exe' if iswin() else 'java'
    user_java = tui.ask(
        f'Полный путь к {java_filename}',
        validate=lambda path: validate_user_java(required_version, path),
        default=default,
    )
    user_java = fix_java_path(user_java)
    return check_java(user_java)


def can_download_java():
    # x86_64 on linux/mac, AMD64 on windows
    return iswin() and platform.machine().lower() in ['x86_64', 'amd64']


async def download_java(required_version: str, target_dir: Path) -> JavaInstall:
    print('Загрузка java...', end='', flush=True)
    params = {
        'java_version': required_version,
        'os': 'windows',
        'arch': 'x64',
        'archive_type': 'zip',
        'java_package_type': 'jre',
        'javafx_bundled': 'false',
        'latest': 'true',
        'release_status': 'ga',
    }
    versions_url = 'https://api.azul.com/metadata/v1/zulu/packages/?' + urlencode(
        params
    )
    client = httpx.AsyncClient()
    resp = await client.get(versions_url)
    resp.raise_for_status()
    versions = resp.json()
    if not versions:
        raise ValueError('No java versions available')
    version_url = versions[0]['download_url']
    with TemporaryFile() as f:
        async with client.stream('GET', version_url) as resp:
            print('\r', end='')
            total = int(resp.headers['Content-Length'])
            with rich.progress.Progress(
                rich.progress.TextColumn('[progress.description]{task.description}'),
                rich.progress.BarColumn(),
                rich.progress.DownloadColumn(),
                rich.progress.TransferSpeedColumn(),
            ) as progress:
                t = progress.add_task('Загрузка java...', total=total)
                async for chunk in resp.aiter_bytes():
                    f.write(chunk)
                    progress.update(t, completed=resp.num_bytes_downloaded)
        f.seek(0)
        zf = ZipFile(f)
        for archive_file_info in track(zf.infolist(), 'Распаковка java...'):
            if archive_file_info.is_dir():
                continue
            with zf.open(archive_file_info.filename) as archived_file:
                target_file_path = target_dir / archive_file_info.filename.split('/', 1)[1]
                target_file_path.parent.mkdir(parents=True, exist_ok=True)
                with open(target_file_path, 'wb') as target_file:
                    target_file.write(archived_file.read())
    res = check_java(target_dir / 'bin' / 'java')
    if not res:
        raise ValueError('Ошибка загрузки java')
    return res


async def find_java(required_version: str, config: Config) -> str:
    if iswin():
        res = find_java_win()
    elif islinux():
        res = find_java_linux()
    elif ismac():
        res = find_java_macos()
    else:
        raise ValueError('Unsupported platform')

    if default_java := check_java('java'):
        res.append(default_java)

    launcher_java_dir = get_data_dir(config) / 'java' / required_version
    if can_download_java():
        launcher_java_path = launcher_java_dir / 'bin' / 'java.exe'
        if launcher_java := check_java(launcher_java_path):
            res.append(launcher_java)

    res = [x for x in res if x and is_good_version(required_version, x)]

    if not res and can_download_java():
        print(f'Java {required_version} не найдена')
        if tui.choice('Скачать автоматически?', [('Да', True), ('Нет', False)]):
            res = [await download_java(required_version, launcher_java_dir)]

    if not res:
        print(f'Java {required_version} не найдена')
        print('Установите ее с https://adoptium.net/ и перезапустите лаунчер')
        print('Если Java на самом деле установлена, введите путь к ней')
        return ask_user_java(required_version).path

    return res[0].path


__all__ = ['find_java', 'check_java', 'ask_user_java', 'fix_java_path']
