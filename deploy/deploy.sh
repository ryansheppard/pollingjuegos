#!/usr/bin/env bash
# Run on the droplet as root after both images for this release have been pushed.
set -euo pipefail

if (( EUID != 0 )); then
  echo "Run as root (for Docker and /opt/pollingjuegos)" >&2
  exit 1
fi
if (( $# != 1 )) || [[ ! $1 =~ ^[0-9a-f]{12}$ ]]; then
  echo "Usage: $0 <12-character commit SHA>" >&2
  exit 2
fi

exec 9>/run/lock/pollingjuegos-deploy.lock
flock -n 9 || { echo "Another deployment is running" >&2; exit 1; }

export RELEASE=$1
app_dir=/opt/pollingjuegos
compose_file=$app_dir/compose.yaml
release_file=$app_dir/.env
compose=(docker compose -f "$compose_file")

# Do all downloads before interrupting the running backend.
"${compose[@]}" pull
"${compose[@]}" stop backend
"${compose[@]}" run --rm --no-deps backend \
  /app/.venv/bin/python manage.py migrate --noinput
"${compose[@]}" up -d
"${compose[@]}" ps

# Keep the active release available for later Compose commands/restarts.
tmp=$(mktemp "$app_dir/.env.XXXXXX")
trap 'rm -f "$tmp"' EXIT
printf 'RELEASE=%s\n' "$RELEASE" > "$tmp"
chmod 0644 "$tmp"
mv -f "$tmp" "$release_file"
echo "Deployed $RELEASE"
