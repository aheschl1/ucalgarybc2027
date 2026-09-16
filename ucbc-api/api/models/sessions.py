from pydantic import BaseModel


class Credentials(BaseModel):
    email: str
    password: str
