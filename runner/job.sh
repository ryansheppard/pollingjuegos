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

main() {
  set -e
  bazelrc="common --color=no --curses=no"
  # Kept in the job dir, which is wiped before the template is checkpointed, so the key
  # never lands on a checkpoint's disk.
  if [[ -f "$dir/buildbuddy-api-key" ]]; then
    cat >"$dir/credential-helper" <<HELPER
#!/bin/sh
# Bazel credential helper protocol: discard the request, answer with headers.
cat > /dev/null
printf '{"headers":{"x-buildbuddy-api-key":["%s"]}}\n' "\$(cat "$dir/buildbuddy-api-key")"
HELPER
    chmod 700 "$dir/credential-helper"
    bazelrc+=$'\n'"common --credential_helper=*.buildbuddy.io=$dir/credential-helper"
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

(main "$@")
rc=$?
rm -rf "$dir/docker" "$dir/buildbuddy-api-key" "$dir/credential-helper" "$dir/github-token"
echo "$rc" >"$dir/exit"
