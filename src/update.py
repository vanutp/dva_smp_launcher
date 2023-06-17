import os
from hashlib import sha1
from pathlib import Path

import httpx
import sys

from rich import print
from rich.progress import Progress

from build_cfg import SERVER_BASE
from src.compat import iswin, ismac, islinux, is_frozen, chmod_x


def get_update_url():
    if iswin():
        return f'{SERVER_BASE}launcher/dva_smp_launcher.exe'
    if ismac():
        return f'{SERVER_BASE}launcher/dva_smp_launcher_macos'
    if islinux():
        return f'{SERVER_BASE}launcher/dva_smp_launcher_linux'
    raise ValueError('Unsupported platform')


async def update_required():
    if not is_frozen():
        return False
    print('Проверка обновлений...', flush=True)
    hash_url = get_update_url() + '.sha1'
    client = httpx.AsyncClient()
    new_hash_resp = await client.get(hash_url)
    new_hash = new_hash_resp.text.strip()
    with open(sys.executable, 'rb') as f:
        current_hash = sha1(f.read()).hexdigest()
    return new_hash != current_hash


async def update_if_required():
    if len(sys.argv) == 3 and sys.argv[1] == 'updated':
        Path(sys.argv[2]).unlink()
        return
    if not await update_required():
        return

    current_file = Path(sys.executable)
    new_file_name = current_file.with_stem(current_file.stem + '_upd')

    client = httpx.AsyncClient()
    with open(new_file_name, 'wb') as f:
        async with client.stream('GET', get_update_url()) as resp:
            total = int(resp.headers['Content-Length'])
            with Progress() as progress:
                t = progress.add_task('Обновление...', total=total)
                async for chunk in resp.aiter_bytes():
                    f.write(chunk)
                    progress.update(t, completed=resp.num_bytes_downloaded)

    renamed_current_file = current_file.with_stem(current_file.stem + '_old')
    current_file.rename(renamed_current_file)
    new_file_name.rename(current_file)
    chmod_x(current_file)

    os.execl(sys.executable, 'updated', renamed_current_file)


__all__ = ['update_if_required']
