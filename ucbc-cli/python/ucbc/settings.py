import os


def server_url() -> str:
    """returns the server url set (UCBC_SERVER) or default"""
    return os.environ.get("UCBC_SERVER", "https://ucbc.andrewheschl.ca/api")
