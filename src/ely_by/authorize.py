import logging
import webbrowser

import httpx
import uvicorn
from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import Response, RedirectResponse
from starlette.routing import Route

from build_cfg import CLIENT_ID, CLIENT_SECRET, APP_NAME

LISTEN_PORT = 18741
REDIRECT_URI = f'http://127.0.0.1:{LISTEN_PORT}/'


async def authorize() -> str:
    print('Авторизуйтесь в открывшемся окне браузера...')
    url = (
        f'https://account.ely.by/oauth2/v1'
        f'?client_id={CLIENT_ID}'
        f'&redirect_uri={REDIRECT_URI}'
        f'&response_type=code'
        f'&scope=account_info%20minecraft_server_session'
        f'&prompt=select_account'
    )
    print(f'Или откройте ссылку вручную: {url}')
    webbrowser.open(url)

    code: str | None = None

    async def handle(request: Request) -> Response:
        nonlocal code
        if 'code' not in request.query_params:
            return Response('"code" query param missing', 400)
        try:
            code = await exchange_code(request.query_params['code'])
        except InvalidCodeError:
            return Response('Неверный код', 400)
        server.should_exit = True
        return RedirectResponse(
            f'https://account.ely.by/oauth2/code/success?appName={APP_NAME}', 302
        )

    app = Starlette(
        routes=[
            Route('/', handle),
        ]
    )

    server_config = uvicorn.Config(app, port=LISTEN_PORT, log_level=logging.WARNING)
    server = uvicorn.Server(config=server_config)
    await server.serve()

    if not code:
        raise ValueError('Server stopped before receiving the code')

    return code


class InvalidCodeError(ValueError):
    ...


async def exchange_code(code: str) -> str:
    client = httpx.AsyncClient()
    token_response = await client.post(
        "https://account.ely.by/api/oauth2/v1/token",
        data={
            "client_id": CLIENT_ID,
            "client_secret": CLIENT_SECRET,
            "redirect_uri": REDIRECT_URI,
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
