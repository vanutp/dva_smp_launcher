import dataclasses
import json
from dataclasses import dataclass
from json import JSONDecodeError
from pathlib import Path

from platformdirs import PlatformDirs

from build_cfg import DATA_DIR_NAME
from src.auth import AuthenticatedUser


@dataclass
class Config:
    token: str = ''
    user_info: AuthenticatedUser = dataclasses.field(default_factory=AuthenticatedUser)
    java_path: dict[str, str] = dataclasses.field(default_factory=dict)
    assets_dir: str = ''
    data_dir: str = ''
    xmx: int = 3072
    modpack: str = ''


def get_dirs():
    return PlatformDirs(
        DATA_DIR_NAME, appauthor=False, ensure_exists=True, roaming=True
    )


def get_config_path():
    return get_dirs().user_config_path / 'config.json'


def get_data_dir(config: Config) -> Path:
    return Path(config.data_dir or get_dirs().user_data_path)


def get_minecraft_dir(config: Config, modpack_name: str) -> Path:
    res = get_data_dir(config) / 'modpacks' / modpack_name
    res.mkdir(parents=True, exist_ok=True)
    return res


def get_index_path(config: Config) -> Path:
    return get_data_dir(config) / 'modpacks' / 'index.json'


def get_assets_dir(config: Config) -> Path:
    return Path(config.assets_dir or (get_data_dir(config) / 'assets'))


def load_config() -> Config:
    config_path = get_config_path()
    if not config_path.is_file():
        return Config()

    try:
        with open(config_path) as f:
            data = json.load(f)
    except JSONDecodeError:
        return Config()

    if 'java_options' in data:
        del data['java_options']
    res = Config(**data)
    if isinstance(res.java_path, str):
        res.java_path = {res.modpack: res.java_path}
    from src.utils.java import fix_java_path
    res.java_path = {name: fix_java_path(path) for name, path in res.java_path.items()}

    if isinstance(res.user_info, dict):
        res.user_info = AuthenticatedUser(**res.user_info)

    if not (
        isinstance(res.user_info, AuthenticatedUser)
        and isinstance(res.java_path, dict)
        and isinstance(res.assets_dir, str)
        and isinstance(res.data_dir, str)
        and isinstance(res.xmx, int)
    ):
        print(res.user_info)
        return Config()

    return res


def save_config(config: Config) -> None:
    with open(get_config_path(), 'w') as f:
        json.dump(dataclasses.asdict(config), f, indent=2)


__all__ = [
    'Config',
    'load_config',
    'save_config',
    'get_minecraft_dir',
    'get_assets_dir',
    'get_data_dir',
]
