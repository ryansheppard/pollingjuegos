#!/bin/sh
# Consistent online SQLite backup; run by pollingjuegos-backup.timer as UID 65532.
set -eu
umask 077
db_path=${POLLINGJUEGOS_DB_PATH:-/var/lib/pollingjuegos/data/db.sqlite3}
backup_dir=${POLLINGJUEGOS_BACKUP_DIR:-/var/backups/pollingjuegos}
# Refuse a missing source rather than letting SQLite create an empty database.
test -f "$db_path"
backup="$backup_dir/db-$(date -u +%Y%m%dT%H%M%SZ).sqlite3"
mkdir -p "$backup_dir"
trap 'rm -f "$backup"' EXIT
sqlite3 -readonly -cmd '.timeout 5000' "$db_path" ".backup '$backup'"
# A zero-length file indicates a failed/incomplete backup.
test -s "$backup"
trap - EXIT
find "$backup_dir" -maxdepth 1 -type f -name 'db-*.sqlite3' -mtime +13 -delete
