#!/usr/bin/env bash
set -euo pipefail
umask 077
script_dir="$(cd "$(dirname "$0")" && pwd)"
if [[ $# -ne 2 ]]; then
  echo "usage: $0 BACKUP.sql.gz.age AGE_IDENTITY_FILE" >&2
  exit 2
fi
backup_file="$1"
identity_file="$2"
: "${RESTORE_DATABASE_URL:?RESTORE_DATABASE_URL must target a disposable *_restore_test database}"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/devstore-restore.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
database="${RESTORE_DATABASE_URL##*/}"
database="${database%%\?*}"
if [[ "$database" != bz_ztore_restore_test ]]; then
  echo "restore target must be the isolated bz_ztore_restore_test database" >&2
  exit 1
fi
MYSQL_URL="$RESTORE_DATABASE_URL" python3 "$script_dir/write_mysql_defaults.py" "$work_dir/client.cnf" bz_ztore_restore_test
age -d -i "$identity_file" "$backup_file" \
  | gzip -dc \
  | mysql --defaults-extra-file="$work_dir/client.cnf" "$database"
echo "Isolated restore completed into $database"
