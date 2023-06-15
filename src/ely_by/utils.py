from dataclasses import dataclass

import httpx


@dataclass
class ElyByUser:
    uuid: str
    username: str
    profile_link: str


async def get_user(token: str) -> ElyByUser:
    client = httpx.AsyncClient()
    resp = await client.get(
        'https://account.ely.by/api/account/v1/info',
        headers={
            'Authorization': f'Bearer {token}',
        },
    )
    resp.raise_for_status()
    data = resp.json()
    return ElyByUser(
        uuid=data['uuid'],
        username=data['username'],
        profile_link=data['profileLink'],
    )
