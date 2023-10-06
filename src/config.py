import dataclasses
import json
from dataclasses import dataclass
from json import JSONDecodeError

from platformdirs import PlatformDirs


@dataclass
class Config:
    token: str = ''
    java_path: str = ''
    assets_dir: str = ''
    xmx: int = 3072


def get_dirs():
    return PlatformDirs('dvasmp', appauthor=False, ensure_exists=True, roaming=True)


def get_config_path():
    return get_dirs().user_config_path / 'config.json'


def get_minecraft_dir(modpack_name: str):
    res = get_dirs().user_data_path / 'modpacks' / modpack_name
    res.mkdir(parents=True, exist_ok=True)
    return res


def load_config() -> Config:
    config_path = get_config_path()
    if not config_path.is_file():
        return Config()

    try:
        with open(config_path) as f:
            data = json.load(f)
    except JSONDecodeError:
        return Config()

    res = Config(**data)
    if not (
        isinstance(res.token, str)
        and isinstance(res.java_path, str)
        and isinstance(res.assets_dir, str)
        and isinstance(res.xmx, int)
    ):
        return Config()

    return res


def save_config(config: Config) -> None:
    with open(get_config_path(), 'w') as f:
        json.dump(dataclasses.asdict(config), f, indent=2)


__all__ = ['Config', 'load_config', 'save_config', 'get_minecraft_dir']
