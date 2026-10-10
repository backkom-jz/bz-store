#!/usr/bin/env python3
import os
import pathlib
import subprocess
import tempfile


writer = pathlib.Path(__file__).with_name("write_mysql_defaults.py")


def run(url, expected):
    with tempfile.TemporaryDirectory() as directory:
        target = pathlib.Path(directory) / "client.cnf"
        result = subprocess.run(
            ["python3", str(writer), str(target), expected],
            env={**os.environ, "MYSQL_URL": url},
            capture_output=True,
            text=True,
            check=False,
        )
        details = (target.stat().st_mode & 0o777, target.read_text()) if target.exists() else None
        return result, details


def main():
    good, details = run("mysql://dev%40store:p%40ss%2Fword@rds.example:3306/bz_ztore", "bz_ztore")
    assert good.returncode == 0, good.stderr
    assert details[0] == 0o600
    content = details[1]
    assert 'user="dev@store"' in content and 'password="p@ss/word"' in content

    for url, expected in (
        ("mysql://user:secret@rds.example:3306/erp_hzs", "bz_ztore"),
        ("mysql://user:secret@rds.example:3306/bz_ztore", "bz_ztore_restore_test"),
        ("postgres://user:secret@rds.example:5432/bz_ztore", "bz_ztore"),
    ):
        rejected, details = run(url, expected)
        assert rejected.returncode != 0
        assert details is None
        assert "secret" not in rejected.stderr


if __name__ == "__main__":
    main()
