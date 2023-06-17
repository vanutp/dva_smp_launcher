import asyncio
import string
import traceback

import inquirer.errors
from rich import print

from src import tui
from src.config import load_config, save_config, Config
from src.ely_by import authorize
from src.ely_by.utils import get_user, ElyByUser
from src.errors import LauncherError
from src.launcher import launch
from src.tui import ensure_tty, ask, clear
from src.update import update_if_required
from src.utils.java import find_java, ask_user_java
from src.utils.modpack import sync_modpack


def validate_memory(mem: str):
    if not (mem and all(x in string.digits for x in mem)):
        raise inquirer.errors.ValidationError(mem, reason='Введите число')
    return True


async def main_menu(user_info: ElyByUser, config: Config):
    while True:
        clear()
        print(f'Вы вошли как [green]{user_info.username}[/green]')
        answer = tui.choice(
            'Выберите опцию',
            [
                ('Играть', 'start'),
                (f'Путь к jabe ({config.java_path or "Не задан"})', 'java_path'),
                (f'Выделенная память ({config.xmx} МиБ)', 'xmx'),
                (
                    f'Путь к ассетам ({config.assets_dir or "По умолчанию"})',
                    'assets_dir',
                ),
                ('Выход', 'exit'),
            ],
        )
        if answer == 'java_path':
            config.java_path = ask_user_java(config.java_path).path
        elif answer == 'xmx':
            config.xmx = int(
                ask(
                    'Выделенная память',
                    validate=validate_memory,
                    default=str(config.xmx),
                )
            )
        elif answer == 'assets_dir':
            config.assets_dir = ask(
                'Путь к ассетам',
                default=str(config.assets_dir),
            )
        elif answer in ['start', 'exit']:
            break
        save_config(config)
    if answer == 'exit':
        return
    elif answer == 'start':
        clear()
        modpack_index = await sync_modpack(config)
        await launch(modpack_index, user_info, config)


async def _main():
    await update_if_required()
    ensure_tty()
    config = load_config()
    if not config.token:
        config.token = await authorize()
        save_config(config)
    if not config.java_path:
        config.java_path = find_java()
        save_config(config)
    user_info = await get_user(config.token)
    await main_menu(user_info, config)


def main():
    try:
        asyncio.run(_main())
    except LauncherError as e:
        print(f'[red]{e.message}[/red]')
        print('[blue]Нажмите Enter чтобы выйти[/blue]')
        input()
    except Exception:
        traceback.print_exc()
        print('[red]Произошла неизвестная ошибка[/red]')
        print('[blue]Нажмите Enter чтобы выйти[/blue]')
        input()
