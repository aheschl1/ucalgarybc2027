"""Blob storage through the S3 API. `UCBC_BLOB_URL` is one URL: scheme and host are the
endpoint, userinfo the access and secret keys, the path the bucket, `?region=` optional."""

import asyncio
from typing import TYPE_CHECKING, Annotated
from urllib.parse import parse_qs, unquote, urlparse

import boto3
from botocore.config import Config
from fastapi import Depends, Request

if TYPE_CHECKING:
    from mypy_boto3_s3 import S3Client


class BlobMissing(Exception):
    pass


class BlobStore:
    def __init__(self, url: str) -> None:
        u = urlparse(url)
        if not (u.scheme and u.hostname and u.username and u.password and u.path.strip("/")):
            raise ValueError("blob url must be scheme://access:secret@host[:port]/bucket")
        endpoint = f"{u.scheme}://{u.hostname}" + (f":{u.port}" if u.port else "")
        self.bucket = u.path.strip("/")
        region = parse_qs(u.query).get("region", ["us-east-1"])[0]
        self._client: S3Client = boto3.client(
            "s3",
            endpoint_url=endpoint,
            aws_access_key_id=unquote(u.username),
            aws_secret_access_key=unquote(u.password),
            region_name=region,
            config=Config(s3={"addressing_style": "path"}),
        )

    async def ensure_bucket(self) -> None:
        """Creates the bucket when it is missing, as it is on a fresh local MinIO."""

        def go() -> None:
            try:
                self._client.create_bucket(Bucket=self.bucket)
            except (
                self._client.exceptions.BucketAlreadyOwnedByYou,
                self._client.exceptions.BucketAlreadyExists,
            ):
                pass

        await asyncio.to_thread(go)

    async def put(self, key: str, data: bytes, content_type: str) -> None:
        await asyncio.to_thread(
            self._client.put_object,
            Bucket=self.bucket,
            Key=key,
            Body=data,
            ContentType=content_type,
        )

    async def get(self, key: str) -> bytes:
        def go() -> bytes:
            try:
                return self._client.get_object(Bucket=self.bucket, Key=key)["Body"].read()
            except self._client.exceptions.NoSuchKey as e:
                raise BlobMissing(key) from e

        return await asyncio.to_thread(go)

    async def delete(self, key: str) -> None:
        await asyncio.to_thread(self._client.delete_object, Bucket=self.bucket, Key=key)


def get_blobs(request: Request) -> BlobStore:
    blobs: BlobStore = request.app.state.blobs
    return blobs


Blobs = Annotated[BlobStore, Depends(get_blobs)]
