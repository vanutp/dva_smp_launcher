from typing import Callable, TypeVar

import inquirer
import sys
from inquirer.render import ConsoleRender

from src.errors import LauncherError


def ensure_tty():
    if not sys.stdout.isatty():
        raise LauncherError('Пожалуйста, запустите лаунчер из консоли')


def ask(
    message: str, *, default: str = None, validate: Callable[[str], bool] = None
) -> str:
    def real_validate(_, val) -> bool:
        if not validate:
            return True
        return validate(val)

    render = ConsoleRender()
    return render.render(
        inquirer.Text(
            'option', message=message, default=default, validate=real_validate
        ),
        {},
    )


T = TypeVar('T')


def choice(message: str, choices: list[tuple[str, T]]) -> T:
    render = ConsoleRender()
    return render.render(inquirer.List('option', message=message, choices=choices), {})


def clear():
    sys.stdout.write('\033c')
    sys.stdout.flush()


__all__ = ['ensure_tty', 'ask', 'choice', 'clear']
