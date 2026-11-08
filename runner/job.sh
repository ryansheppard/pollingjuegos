#!/usr/bin/env bash
# Runs on the MicroVM, uploaded and started detached by vm-run.sh because exec commands
# time out after 60s.
#
# Usage: job.sh <job-dir> <repo-url> <commit-sha> <command...>
set -uo pipefail

dir=$1
repo_url=$2
sha=$3
shift 3
echo $$ >"$dir/pid"

# Shared by every job dir under the same parent, so jobs on one VM run one at a time.
exec 9>"${dir%/*}.lock"
echo "Waiting for the VM build lock..."
flock 9

main() {
  set -e
  bazelrc="common --color=no --curses=no"
  if [[ -f "$dir/buildbuddy-api-key" ]]; then
    install -D -m 600 "$dir/buildbuddy-api-key" ~/.config/buildbuddy/api-key
    cat >~/.config/buildbuddy/credential-helper <<'HELPER'
#!/bin/sh
# Bazel credential helper protocol: discard the request, answer with headers.
cat > /dev/null
printf '{"headers":{"x-buildbuddy-api-key":["%s"]}}\n' "$(cat "$HOME/.config/buildbuddy/api-key")"
HELPER
    chmod 700 ~/.config/buildbuddy/credential-helper
    bazelrc+=$'\n'"common --credential_helper=*.buildbuddy.io=$HOME/.config/buildbuddy/credential-helper"
  fi
  printf '%s\n' "$bazelrc" >~/.bazelrc

  [[ -d /src/pollingjuegos/.git ]] || git clone --quiet --depth 1 "$repo_url" /src/pollingjuegos
  cd /src/pollingjuegos
  git fetch --quiet --depth 1 origin "$sha"
  git checkout --quiet --force FETCH_HEAD
  echo "Checked out $(git log --oneline -1)"
  mise install bazel

  if [[ -f "$dir/docker/config.json" ]]; then
    export DOCKER_CONFIG="$dir/docker"
  fi
  if [[ -f "$dir/github-token" ]]; then
    GH_TOKEN=$(cat "$dir/github-token")
    export GH_TOKEN
  fi
  "$@"
}

# The Bazel server daemonizes and outlives the job; if it inherited the lock's fd it
# would hold the lock forever, so the command runs with that fd closed.
(main "$@") 9>&-
rc=$?
rm -rf "$dir/docker" "$dir/buildbuddy-api-key" "$dir/github-token"
echo "$rc" >"$dir/exit"
