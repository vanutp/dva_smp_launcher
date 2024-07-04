from abc import ABC, abstractmethod
from dataclasses import dataclass
import build_cfg


@dataclass
class AuthenticatedUser:
    uuid: str = ''
    username: str = ''


class UnauthorizedException(Exception):
    pass


class AuthProvider(ABC):
    @abstractmethod
    async def authenticate(self) -> str: ...

    @abstractmethod
    async def get_user(self, token: str) -> AuthenticatedUser: ...

    @staticmethod
    def get() -> 'AuthProvider':
        if tgauth_base := getattr(build_cfg, 'TGAUTH_BASE', None):
            from .tgauth import TGAuthProvider

            return TGAuthProvider(tgauth_base)
        elif (
            (client_id := getattr(build_cfg, 'ELYBY_CLIENT_ID', None))
            and (client_secret := getattr(build_cfg, 'ELYBY_CLIENT_SECRET', None))
            and (app_name := getattr(build_cfg, 'ELYBY_APP_NAME', None))
        ):
            from .ely_by import ElyByProvider

            return ElyByProvider(client_id, client_secret, app_name)
        else:
            raise ValueError(
                'Launcher misconfigured: could not determine authentication provider'
            )
