import asyncio
import os.path
import shlex
from subprocess import Popen

from rich import print

from src.compat import iswin, ismac
from src.config import Config, get_minecraft_dir
from src.ely_by.utils import ElyByUser
from src.errors import LauncherError
from src.utils.modpack import ModpackIndex, get_assets_dir

AUTHLIB_INJECTOR_FILENAME = 'authlib-injector.jar'
CLIENT_FILENAME = 'client.jar'
# from LL (formerly TL) launcher
GC_OPTIONS = [
    '-XX:+UnlockExperimentalVMOptions',
    '-XX:+UseG1GC',
    '-XX:G1NewSizePercent=20',
    '-XX:G1ReservePercent=20',
    '-XX:MaxGCPauseMillis=50',
    '-XX:G1HeapRegionSize=32M',
    '-XX:+DisableExplicitGC',
    '-XX:+AlwaysPreTouch',
    '-XX:+ParallelRefProcEnabled',
]


def apply_arg(arg: dict) -> bool:
    if arg['value'] == ['-Dos.name=Windows 10', '-Dos.version=10.0']:
        return False

    if 'rules' not in arg:
        return True

    rules = arg['rules']
    assert len(rules) == 1
    rules = rules[0]

    if rules['action'] != 'allow':
        return False
    if 'os' in rules:
        if 'name' not in rules['os']:
            return False
        os_name = rules['os']['name']
        if os_name == 'windows' and iswin():
            return True
        elif os_name == 'osx' and ismac():
            return True
    elif 'features' in rules:
        if rules['features'].get('has_custom_resolution', False):
            return True
    return False


def replace_launch_config_variables(argument: str, variables: dict[str, str]):
    for k, v in variables.items():
        argument = argument.replace('${' + k + '}', v)
    return argument


async def launch(modpack_index: ModpackIndex, user_info: ElyByUser, config: Config):
    print('[green]Запуск![/green]', flush=True)
    mc_dir = get_minecraft_dir(modpack_index.modpack_name)
    (mc_dir / 'natives').mkdir(exist_ok=True)

    if modpack_index.classpath:
        classpath = [
            str(mc_dir / x) for x in modpack_index.classpath
        ]
    else:
        classpath = [
            str(mc_dir / x) for x in modpack_index.objects if x.split('/')[0] == 'libraries'
        ]
        classpath.append(str(mc_dir / CLIENT_FILENAME))

    variables = {
        'natives_directory': str(mc_dir / 'natives'),
        'launcher_name': 'java-minecraft-launcher',
        'launcher_version': '1.6.84-j',
        'classpath': os.pathsep.join(classpath),
        'classpath_separator': os.pathsep,
        'library_directory': str(mc_dir / 'libraries'),
        'auth_player_name': user_info.username,
        'version_name': modpack_index.version,
        'game_directory': str(mc_dir),
        'assets_root': str(get_assets_dir(config)),
        'assets_index_name': modpack_index.asset_index,
        'auth_uuid': user_info.uuid.replace('-', ''),
        'auth_access_token': config.token,
        'clientid': '',
        'auth_xuid': '',
        'user_type': 'mojang',
        'version_type': 'release',
        'resolution_width': '925',
        'resolution_height': '530',
    }

    java_options = [
        f'-javaagent:{mc_dir / AUTHLIB_INJECTOR_FILENAME}=ely.by',
        *GC_OPTIONS,
        '-Xms512M',
        f'-Xmx{config.xmx}M',
        '-Duser.language=en',
        '-Dfile.encoding=UTF-8',
        *shlex.split(config.java_options),
    ]
    for arg in modpack_index.java_args:
        if not isinstance(arg['value'], list):
            arg['value'] = [arg['value']]

        if apply_arg(arg):
            java_options.extend(
                [replace_launch_config_variables(x, variables) for x in arg['value']]
            )

    minecraft_options = []
    for arg in modpack_index.game_args:
        if not isinstance(arg['value'], list):
            arg['value'] = [arg['value']]

        if apply_arg(arg):
            minecraft_options.extend(
                [replace_launch_config_variables(x, variables) for x in arg['value']]
            )

    command = [
        config.java_path,
        *java_options,
        modpack_index.main_class,
        *minecraft_options,
    ]
    p = Popen(command, start_new_session=True, cwd=str(mc_dir))
    await asyncio.sleep(3)
    if p.poll() is not None:
        raise LauncherError('Процесс майнкрафта завершился слишком быстро...')
