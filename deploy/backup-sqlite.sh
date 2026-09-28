#!/bin/sh
# Consistent online SQLite backup; run by pollingjuegos-backup.timer as the pollingjuegos user.
set -eu
umask 077
backup_dir=/var/backups/pollingjuegos
backup="$backup_dir/db-$(date -u +%Y%m%dT%H%M%SZ).sqlite3"
mkdir -p "$backup_dir"
trap 'rm -f "$backup"' EXIT
sqlite3 /opt/pollingjuegos/db.sqlite3 ".backup '$backup'"
# A zero-length file indicates a failed/incomplete backup.
test -s "$backup"
trap - EXIT
find "$backup_dir" -maxdepth 1 -type f -name 'db-*.sqlite3' -mtime +13 -delete
