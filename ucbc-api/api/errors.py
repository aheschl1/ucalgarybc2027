"""Errors raised by the services; the app turns them into responses, the CLI into messages."""


class ApiError(Exception):
    status_code = 400


class Forbidden(ApiError):
    status_code = 403


class NotFound(ApiError):
    status_code = 404


class Conflict(ApiError):
    status_code = 409


class PayloadTooLarge(ApiError):
    status_code = 413
