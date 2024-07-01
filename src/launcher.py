import asyncio
import os.path
import shlex
from subprocess import Popen, PIPE, STDOUT

from rich import print

import build_cfg
from src.auth import AuthenticatedUser, AuthProvider, ElyByProvider
from src.auth.tgauth import TGAuthProvider
from src.compat import iswin, ismac, win_pipe_nowait, islinux
from src.config import Config, get_minecraft_dir
from src.errors import LauncherError
from src.utils.modpack import ModpackIndex, get_assets_dir

AUTHLIB_INJECTOR_FILENAME = 'authlib-injector.jar'
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
    if arg.get('value') == ['-Dos.name=Windows 10', '-Dos.version=10.0']:
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
        elif os_name == 'linux' and islinux():
            return True
    elif 'features' in rules:
        if rules['features'].get('has_custom_resolution', False):
            return True
    return False


def replace_launch_config_variables(argument: str, variables: dict[str, str]):
    for k, v in variables.items():
        argument = argument.replace('${' + k + '}', v)
    return argument


def library_name_to_path(full_name: str) -> str:
    if full_name.count(':') != 3:
        full_name += ':'
    pkg, name, version, suffix = full_name.split(':')
    pkg = pkg.replace('.', '/')
    suffix = '-' + suffix if suffix else ''
    return f'libraries/{pkg}/{name}/{version}/{name}-{version}{suffix}.jar'


async def launch(
        modpack_index: ModpackIndex, user_info: AuthenticatedUser, config: Config
):
    print('[green]Запуск![/green]', flush=True)
    mc_dir = get_minecraft_dir(modpack_index.modpack_name)
    (mc_dir / 'natives').mkdir(exist_ok=True)

    classpath = []
    for arg in modpack_index.libraries:
        if arg.get('downloadOnly'):
            continue
        if apply_arg(arg):
            classpath.append(str(mc_dir / library_name_to_path(arg['name'])))
    classpath.append(str(mc_dir / modpack_index.client_filename))

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
        *GC_OPTIONS,
        '-Xms512M',
        f'-Xmx{config.xmx}M',
        '-Duser.language=en',
        '-Dfile.encoding=UTF-8',
        *shlex.split(config.java_options),
    ]

    auth_provider = AuthProvider.get()
    if isinstance(auth_provider, ElyByProvider):
        java_options.insert(
            0, f'-javaagent:{mc_dir / AUTHLIB_INJECTOR_FILENAME}=ely.by'
        )
    elif isinstance(auth_provider, TGAuthProvider):
        java_options.insert(
            0,
            f'-javaagent:{mc_dir / AUTHLIB_INJECTOR_FILENAME}={build_cfg.TGAUTH_BASE}',
        )

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
        config.java_path[modpack_index.modpack_name],
        *java_options,
        modpack_index.main_class,
        *minecraft_options,
    ]

    kwargs = {}
    if iswin():
        flags = 0
        flags |= 0x00000008  # DETACHED_PROCESS
        # idk if this is needed
        flags |= 0x00000200  # CREATE_NEW_PROCESS_GROUP
        kwargs['creationflags'] = flags
        kwargs['stdout'] = PIPE
        kwargs['stderr'] = STDOUT

    p = Popen(command, start_new_session=True, cwd=str(mc_dir), **kwargs)
    await asyncio.sleep(3)
    if (return_code := p.poll()) is not None:
        if iswin():
            win_pipe_nowait(p.stdout.fileno())
            print(p.stdout.read().decode())
        raise LauncherError(
            f'Процесс майнкрафта завершился слишком быстро... Код завершения: {return_code}'
        )
