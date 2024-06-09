"""
Most of the paths are from https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaUtils.cpp
"""

import os.path
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import inquirer.errors

from src.compat import ismac, islinux, iswin
from src.tui import ask

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
    return java.version == required_version or java.version.startswith(f'{required_version}.')


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
            exe_path = os.path.join(java_dir_value, 'bin', 'javaw.exe')
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
        if subdir.is_file() or subdir.is_symlink():
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

def ask_user_java(required_version: str, default: str = None) -> (JavaInstall | None):
    user_java = ask('Полный путь к java (javaw.exe на Windows)', validate=lambda path: validate_user_java(required_version, path), default=default)
    return check_java(user_java)


def find_java(required_version: str) -> str:
    if iswin():
        res = find_java_win()
    elif islinux():
        res = find_java_linux()
    elif ismac():
        res = find_java_macos()
    else:
        raise ValueError('Unsupported platform')

    default_java_path = 'javaw' if iswin() else 'java'
    if default_java_path and (default_java := check_java(default_java_path)):
        res.append(default_java)

    res = [x for x in res if x and is_good_version(required_version, x)]
    if not res:
        print(f'Java {required_version} не найдена')
        print('Установите ее с https://adoptium.net/ и перезапустите лаунчер')
        print('Если Java на самом деле установлена, введите путь к ней')
        return ask_user_java(required_version).path

    return res[0].path


__all__ = ['find_java', 'check_java', 'ask_user_java']
