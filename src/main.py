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
from src.utils.modpack import sync_modpack, ModpackNotFoundError, load_indexes, ModpackIndex


def validate_memory(mem: str):
    if not (mem and all(x in string.digits for x in mem)):
        raise inquirer.errors.ValidationError(mem, reason='Введите число')
    return True


async def select_modpack(indexes: list[ModpackIndex]):
    if len(indexes) == 1:
        return indexes[0].modpack_name
    return tui.choice(
        'Выберите сборку', [(x.modpack_name, x.modpack_name) for x in indexes]
    )


async def main_menu(indexes: list[ModpackIndex], user_info: ElyByUser, config: Config):
    print('Загрузка...', end='', flush=True)
    while True:
        clear()
        print(f'Вы вошли как [green]{user_info.username}[/green]')
        select_modpack_entry = [(f'Изменить сборку (выбрана {config.modpack})', 'change_modpack')] if len(indexes) > 1 else []
        answer = tui.choice(
            'Выберите опцию',
            [
                ('Играть', 'start'),
                *select_modpack_entry,
                (f'Путь к Java ({config.java_path or "Не задан"})', 'java_path'),
                (f'Выделенная память ({config.xmx} МиБ)', 'xmx'),
                (
                    f'Путь к ассетам ({config.assets_dir or "По умолчанию"})',
                    'assets_dir',
                ),
                (
                    f'Дополнительные опции Java {f"({config.java_options})" if config.java_options else ""}',
                    'java_options',
                ),
                ('Выход', 'exit'),
            ],
        )
        if answer == 'start':
            clear()
            try:
                modpack_index = await sync_modpack(config)
            except ModpackNotFoundError:
                indexes = await load_indexes()
                config.modpack = await select_modpack(indexes)
            else:
                await launch(modpack_index, user_info, config)
                break
        elif answer == 'change_modpack':
            config.modpack = await select_modpack(indexes)
        elif answer == 'java_path':
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
        elif answer == 'java_options':
            config.java_options = ask(
                'Дополнительные опции Java',
                default=config.java_options,
            )
        elif answer == 'exit':
            break
        save_config(config)


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
    indexes = await load_indexes()
    if not config.modpack:
        config.modpack = await select_modpack(indexes)
        print(config.modpack)
        save_config(config)
    await main_menu(indexes, user_info, config)


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
