import asyncio

from argon2 import PasswordHasher
from argon2.exceptions import VerificationError

_hasher = PasswordHasher()


def hash_password(password: str) -> str:
    return _hasher.hash(password)


async def verify_password(password_hash: str, password: str) -> bool:
    """Argon2 is deliberately slow, so it runs off the event loop."""

    def check() -> bool:
        try:
            return _hasher.verify(password_hash, password)
        except VerificationError:
            return False

    return await asyncio.to_thread(check)
