"""
Most of the paths are from https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaUtils.cpp
"""

import os.path
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import sys

if sys.platform == 'win32':
    from winreg import (
        OpenKeyEx,
        HKEY_LOCAL_MACHINE,
        KEY_READ,
        KEY_ENUMERATE_SUB_KEYS,
        EnumKey,
        CloseKey,
        QueryValueEx,
    )

REQUIRED_JAVA = '17'


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
    if not os.path.isfile(path):
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


def find_java_in_registry(
    key_name: str, subkey_suffix: str, java_dir_key: str
) -> list[JavaInstall]:
    try:
        key = OpenKeyEx(
            HKEY_LOCAL_MACHINE, key_name, access=KEY_READ | KEY_ENUMERATE_SUB_KEYS
        )
    except OSError:
        print(f'Key "{key_name}" not found')
        return []

    subkeys = []
    while True:
        i = 0
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


def find_java_in_dir(dir_: str, *, suffix: str = '', startswith: str = '') -> list[JavaInstall]:
    suffix = Path(suffix)
    res = []
    for subdir in Path(dir_).glob('*'):
        if subdir.is_file() or subdir.is_symlink():
            continue
        if startswith and not subdir.name.startswith(startswith):
            continue
        res.append(check_java(subdir / suffix / 'bin' / 'java'))
    res = [x for x in res if x]
    return res


def find_java_linux() -> list[JavaInstall]:
    res = []
    res.extend(find_java_in_dir('/usr/java'))
    res.extend(find_java_in_dir('/usr/lib/jvm'))
    res.extend(find_java_in_dir('/usr/lib64/jvm'))
    res.extend(find_java_in_dir('/usr/lib32/jvm'))
    res.extend(find_java_in_dir('/opt/jdk'))
    return res


def find_java_macos() -> list[JavaInstall]:
    res = []
    res.extend(find_java_in_dir('/Library/Java/JavaVirtualMachines', suffix='Contents/Home'))
    res.extend(find_java_in_dir('/System/Library/Java/JavaVirtualMachines', suffix='Contents/Home'))
    res.extend(find_java_in_dir('/usr/local/opt', startswith='openjdk'))
    res.extend(find_java_in_dir('/opt/homebrew/opt', startswith='openjdk'))
    return res


def find_java() -> str:
    if sys.platform == 'win32':
        res = find_java_win()
    elif sys.platform == 'linux':
        res = find_java_linux()
    elif sys.platform == 'darwin':
        res = find_java_macos()
    else:
        raise ValueError('Unsupported platform')

    default_java_path = shutil.which('javaw' if sys.platform == 'win32' else 'java')
    if default_java_path and (default_java := check_java(default_java_path)):
        res.append(default_java)

    print(*res, sep='\n')
    res = [x for x in res if x.version == '17' or x.version.startswith('17.')]
    if not res:
        print(
            'Jaba не найдена, установите ее с https://adoptium.net/ и перезапустите лаунчер'
        )
        print('Если jaba на самом деле установлена, введите путь к ней ниже')
        return ''

    return res[0].path


__all__ = ['find_java', 'check_java']
