import webbrowser
from dataclasses import dataclass

import httpx
from httpx import AsyncClient
from qrcode.main import QRCode

from .base import AuthProvider, AuthenticatedUser, UnauthorizedException


@dataclass
class LoginStartResponse:
    code: str
    intermediate_token: str


class TGAuthProvider(AuthProvider):
    def __init__(self, base_url: str, bot_name: str):
        self.client = AsyncClient(base_url=base_url)
        self.bot_name = bot_name

    async def authenticate(self) -> str:
        start_resp = await self.client.post('/login/start')
        start_resp.raise_for_status()
        start_resp = LoginStartResponse(**start_resp.json())
        tg_deeplink = f'https://t.me/{self.bot_name}?start={start_resp.code}'
        webbrowser.open(tg_deeplink)
        print('Нажмите start в боте')
        print('Или отсканируйте QR код:')
        qr = QRCode()
        qr.add_data(tg_deeplink)
        qr.print_ascii(tty=True)
        print(f'Или введите код в бота @{self.bot_name} вручную: {start_resp.code}')

        done = False
        while not done:
            try:
                poll_resp = await self.client.post(
                    '/login/poll',
                    json={'intermediate_token': start_resp.intermediate_token},
                    timeout=60,
                )
            except httpx.TimeoutException:
                continue
            poll_resp.raise_for_status()
            poll_resp = poll_resp.json()
            done = True

        return poll_resp['user']['access_token']

    async def get_user(self, token: str) -> AuthenticatedUser:
        resp = await self.client.get(
            '/login/profile',
            headers={
                'Authorization': f'Bearer {token}',
            },
        )
        if resp.status_code in [401, 403]:
            raise UnauthorizedException()
        resp.raise_for_status()
        data = resp.json()
        return AuthenticatedUser(
            uuid=data['uuid'],
            username=data['username'],
        )
