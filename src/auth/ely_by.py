import asyncio
import logging
import webbrowser

import httpx
import uvicorn
from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import Response, RedirectResponse
from starlette.routing import Route

from .base import AuthProvider, AuthenticatedUser, UnauthorizedException


class InvalidCodeError(ValueError):
    pass


class ElyByProvider(AuthProvider):
    redirect_uri: str | None
    token: str | None

    def __init__(self, client_id: str, client_secret: str, app_name: str):
        self.client_id = client_id
        self.client_secret = client_secret
        self.app_name = app_name
        self.redirect_uri = None
        self.token = None

    async def authenticate(self) -> str:
        async def handle(request: Request) -> Response:
            if 'code' not in request.query_params:
                return Response('"code" query param missing', 400)
            try:
                self.token = await self.exchange_code(request.query_params['code'])
            except InvalidCodeError:
                return Response('Неверный код', 400)
            server.should_exit = True
            return RedirectResponse(
                f'https://account.ely.by/oauth2/code/success?appName={self.app_name}',
                302,
            )

        app = Starlette(
            routes=[
                Route('/', handle),
            ]
        )

        server_config = uvicorn.Config(app, port=0, log_level=logging.WARNING)
        server = uvicorn.Server(config=server_config)

        server_task = asyncio.create_task(server.serve())

        while not server.started:
            await asyncio.sleep(0.1)
        port = server.servers[0].sockets[0].getsockname()[1]
        self.redirect_uri = f'http://localhost:{port}/'
        self.print_auth_url()

        await server_task

        if not self.token:
            raise ValueError('Server stopped before receiving the code')

        return self.token

    async def get_user(self, token: str) -> AuthenticatedUser:
        client = httpx.AsyncClient()
        resp = await client.get(
            'https://account.ely.by/api/account/v1/info',
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

    def print_auth_url(self) -> None:
        if self.redirect_uri is None:
            raise ValueError('redirect_uri is not set')
        print('Авторизуйтесь в открывшемся окне браузера...')
        url = (
            f'https://account.ely.by/oauth2/v1'
            f'?client_id={self.client_id}'
            f'&redirect_uri={self.redirect_uri}'
            f'&response_type=code'
            f'&scope=account_info%20minecraft_server_session'
            f'&prompt=select_account'
        )
        print(f'Или откройте ссылку вручную: {url}')
        webbrowser.open(url)

    async def exchange_code(self, code: str) -> str:
        client = httpx.AsyncClient()
        token_response = await client.post(
            "https://account.ely.by/api/oauth2/v1/token",
            data={
                "client_id": self.client_id,
                "client_secret": self.client_secret,
                "redirect_uri": self.redirect_uri,
                "grant_type": "authorization_code",
                "code": code,
            },
        )
        data = token_response.json()
        if token_response.status_code != 200:
            assert data['error'] == 'invalid_request'
            raise InvalidCodeError
        assert data['token_type'] == 'Bearer'
        return data['access_token']
