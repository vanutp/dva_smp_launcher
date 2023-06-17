import asyncio
import os.path
from subprocess import Popen

from rich import print

from src.compat import iswin
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


async def launch(modpack_index: ModpackIndex, user_info: ElyByUser, config: Config):
    print('[green]Запуск![/green]', flush=True)
    mc_dir = get_minecraft_dir()
    (mc_dir / 'natives').mkdir(exist_ok=True)
    java_options = [
        f'-javaagent:{mc_dir / AUTHLIB_INJECTOR_FILENAME}=ely.by',
        *GC_OPTIONS,
        '-Xms512M',
        f'-Xmx{config.xmx}M',
        '-Duser.language=en',
        '-Dfile.encoding=UTF-8',
        f'-Djava.library.path={mc_dir / "natives"}',
    ]
    if iswin():
        java_options.append(
            '-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump'
        )
    libraries = [str(mc_dir / x) for x in modpack_index.objects if x.split('/')[0] == 'libraries']
    libraries.append(str(mc_dir / CLIENT_FILENAME))
    minecraft_options = [
        '--username',
        user_info.username,
        '--version',
        modpack_index.version,
        '--gameDir',
        str(mc_dir),
        '--assetsDir',
        str(get_assets_dir(config)),
        '--assetIndex',
        modpack_index.asset_index,
        '--uuid',
        user_info.uuid.replace('-', ''),
        '--accessToken',
        config.token,
        '--clientId',
        '',
        '--xuid',
        '',
        '--userType',
        'mojang',
        '--versionType',
        'release',
        '--width',
        '925',
        '--height',
        '530',
    ]
    command = [
        config.java_path,
        *java_options,
        '-cp',
        os.pathsep.join(libraries),
        modpack_index.main_class,
        *minecraft_options,
    ]
    p = Popen(command, start_new_session=True, cwd=str(mc_dir))
    await asyncio.sleep(3)
    if p.poll() is not None:
        raise LauncherError('Процесс майнкрафта завершился слишком быстро...')
