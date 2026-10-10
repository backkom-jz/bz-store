#!/usr/bin/env bash
set -euo pipefail
umask 077
script_dir="$(cd "$(dirname "$0")" && pwd)"
: "${DATABASE_URL:?DATABASE_URL is required}"
: "${AGE_RECIPIENT:?AGE_RECIPIENT is required}"
backup_dir="${BACKUP_DIR:-./backups}"
mkdir -p "$backup_dir"
chmod 700 "$backup_dir"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/devstore-backup.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
MYSQL_URL="$DATABASE_URL" python3 "$script_dir/write_mysql_defaults.py" "$work_dir/client.cnf" bz_ztore
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output="$backup_dir/devstore-$timestamp.sql.gz.age"
mysqldump --defaults-extra-file="$work_dir/client.cnf" --single-transaction --hex-blob --routines --triggers --events --set-gtid-purged=OFF bz_ztore \
  | gzip -c \
  | age -r "$AGE_RECIPIENT" -o "$work_dir/backup.age"
mv "$work_dir/backup.age" "$output"
chmod 600 "$output"
echo "Encrypted backup written: $output"
