#!/usr/bin/env python3
"""Write a narrowly scoped, mode-0600 MySQL client file without printing secrets."""
import os
import pathlib
import sys
from urllib.parse import unquote, urlsplit


def fail():
    raise SystemExit("refusing unsafe MySQL backup target or malformed connection URL")


if len(sys.argv) != 3:
    fail()
target, expected_database = sys.argv[1:]
raw = os.environ.get("MYSQL_URL", "")
try:
    url = urlsplit(raw)
    database = unquote(url.path.removeprefix("/"))
    username = unquote(url.username or "")
    password = unquote(url.password or "")
    host = url.hostname or ""
    port = url.port or 3306
except ValueError:
    fail()
if (
    url.scheme != "mysql"
    or database != expected_database
    or not host
    or not username
    or not password
    or any(c in value for value in (username, password, host) for c in "\r\n\x00")
):
    fail()


def quoted(value):
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


content = "[client]\n" + "\n".join(
    (
        "protocol=tcp",
        "host=" + quoted(host),
        "port=" + str(port),
        "user=" + quoted(username),
        "password=" + quoted(password),
        "database=" + quoted(database),
        "default-character-set=utf8mb4",
    )
) + "\n"
path = pathlib.Path(target)
path.write_text(content, encoding="utf-8")
path.chmod(0o600)
