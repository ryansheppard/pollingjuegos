#!/usr/bin/env bash
# Runs a Bazel command for one commit on the runner MicroVM and streams its output.
#
# Usage: runner/vm-bazel.sh [--registry-push] <microvm-id> <commit-sha> -- <bazel args...>
#
# Environment:
#   DIGITALOCEAN_ACCESS_TOKEN  token for doctl (DIGITALOCEAN_TOKEN is used as a fallback)
#   BUILDBUDDY_API_KEY         optional; handed to Bazel through a credential helper
#   VM_BAZEL_REPO_URL          optional; defaults to this repository on GitHub
set -euo pipefail

usage() {
  echo "usage: $0 [--registry-push] <microvm-id> <commit-sha> -- <bazel args...>" >&2
  exit 2
}

registry_push=false
if [[ ${1:-} == --registry-push ]]; then
  registry_push=true
  shift
fi
(($# >= 4)) && [[ $3 == -- ]] || usage
vm=$1
sha=$2
shift 3
[[ $sha =~ ^[0-9a-f]{40}$ ]] || {
  echo "commit must be a full 40-character SHA" >&2
  exit 2
}

repo_url=${VM_BAZEL_REPO_URL:-https://github.com/ryansheppard/pollingjuegos}
poll_seconds=10
# exec output is capped at 4 MiB, so each poll fetches at most this much log.
chunk_bytes=$((3 * 1024 * 1024))
max_poll_failures=6

# doctl reads DIGITALOCEAN_ACCESS_TOKEN itself, which keeps the token off its command line.
if [[ -z ${DIGITALOCEAN_ACCESS_TOKEN:-} && -n ${DIGITALOCEAN_TOKEN:-} ]]; then
  export DIGITALOCEAN_ACCESS_TOKEN=$DIGITALOCEAN_TOKEN
fi

run_id=${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-0}-$$-$(date +%s)
dir=/tmp/vm-bazel/$run_id

# The poll loop does its own backing off, so doctl's retries would only multiply requests,
# and the timeout keeps a hung API call from blocking Ctrl-C (bash waits for the child).
remote() { timeout 90 doctl --http-retry-max 1 compute microvm exec "$vm" -- bash -c "$1"; }

mask() {
  if [[ -n ${GITHUB_ACTIONS:-} ]]; then
    echo "::add-mask::$1"
  fi
}

# Secrets travel base64-encoded inside the exec request, never as plain arguments to
# anything that echoes them; the encoded form is masked too since Actions only masks
# the raw secret value.
put_secret() {
  local path=$1 value=$2 encoded
  encoded=$(printf '%s' "$value" | base64 -w0)
  mask "$encoded"
  remote "umask 077 && mkdir -p \"\$(dirname $path)\" && printf %s $encoded | base64 -d > $path"
}

# The job script runs detached on the VM because exec commands time out after 60s.
job_script() {
  local bazel_args
  bazel_args=$(printf '%q ' "$@")
  cat <<EOF
#!/usr/bin/env bash
set -uo pipefail
dir=$dir
echo \$\$ > "\$dir/pid"

exec 9>/tmp/vm-bazel.lock
echo "Waiting for the VM build lock..."
flock 9

main() {
	set -e
	bazelrc="common --color=no --curses=no"
	if [[ -f "\$dir/buildbuddy-api-key" ]]; then
		install -D -m 600 "\$dir/buildbuddy-api-key" ~/.config/buildbuddy/api-key
		cat > ~/.config/buildbuddy/credential-helper <<'HELPER'
#!/bin/sh
# Bazel credential helper protocol: discard the request, answer with headers.
cat > /dev/null
printf '{"headers":{"x-buildbuddy-api-key":["%s"]}}\n' "\$(cat "\$HOME/.config/buildbuddy/api-key")"
HELPER
		chmod 700 ~/.config/buildbuddy/credential-helper
		bazelrc+=\$'\n'"common --credential_helper=*.buildbuddy.io=\$HOME/.config/buildbuddy/credential-helper"
	fi
	printf '%s\n' "\$bazelrc" > ~/.bazelrc

	[[ -d /src/pollingjuegos/.git ]] || git clone --quiet --depth 1 $(printf '%q' "$repo_url") /src/pollingjuegos
	cd /src/pollingjuegos
	git fetch --quiet --depth 1 origin $sha
	git checkout --quiet --force FETCH_HEAD
	echo "Checked out \$(git log --oneline -1)"
	mise install bazel

	if [[ -f "\$dir/docker/config.json" ]]; then
		export DOCKER_CONFIG="\$dir/docker"
	fi
	bazel $bazel_args
}

(main)
rc=\$?
rm -rf "\$dir/docker" "\$dir/buildbuddy-api-key"
echo "\$rc" > "\$dir/exit"
EOF
}

finished=false
cleanup() {
  # A second Ctrl-C during cleanup should abandon it, not restart it.
  trap - INT TERM
  if [[ $finished != true ]]; then
    echo "Stopping the remote job" >&2
    remote "test -f $dir/pid && kill -TERM -- -\$(cat $dir/pid)" || true
  fi
  remote "rm -rf $dir" || true
  # One attempt only: the idle timer pauses it anyway if this fails.
  echo "Pausing the MicroVM"
  timeout 30 doctl --http-retry-max 0 compute microvm pause "$vm" >/dev/null || echo "Pausing failed; the idle timer will pause it" >&2
}
trap cleanup EXIT
# Bash skips EXIT traps when killed by these, and Actions sends them on cancel.
trap 'exit 130' INT
trap 'exit 143' TERM

vm_state() { timeout 30 doctl compute microvm get "$vm" --format State --no-header | tr -d '[:space:]'; }

# exec does not wake a paused MicroVM; it hangs until the API's proxy times out.
if [[ $(vm_state) != running ]]; then
  echo "Resuming the MicroVM"
  timeout 60 doctl compute microvm resume "$vm" >/dev/null
  for _ in {1..24}; do
    [[ $(vm_state) == running ]] && break
    sleep 5
  done
  [[ $(vm_state) == running ]] || {
    echo "The MicroVM did not reach running (state: $(vm_state))" >&2
    exit 1
  }
fi

remote "umask 077 && mkdir -p $dir"
if [[ -n ${BUILDBUDDY_API_KEY:-} ]]; then
  put_secret "$dir/buildbuddy-api-key" "$BUILDBUDDY_API_KEY"
fi
if [[ $registry_push == true ]]; then
  registry_config=$(doctl registry docker-config --read-write --expiry-seconds 1800)
  put_secret "$dir/docker/config.json" "$registry_config"
fi

script=$(job_script "$@" | base64 -w0)
remote "printf %s $script | base64 -d > $dir/job.sh && setsid nohup bash $dir/job.sh > $dir/log 2>&1 < /dev/null &"

# Each poll returns "<bytes sent through> <full log size> <exit code or ->", then the
# log bytes. Reading the exit file first means a set exit code covers the whole log.
offset=0
failures=0
while :; do
  if ! out=$(remote "cd $dir && e=- && { [[ -f exit ]] && e=\$(cat exit); }
		full=\$(stat -c %s log 2>/dev/null || echo 0)
		end=\$(( full < $offset + $chunk_bytes ? full : $offset + $chunk_bytes ))
		echo \"\$end \$full \$e\"
		tail -c +$((offset + 1)) log | head -c \$(( end - $offset ))
		printf x"); then
    failures=$((failures + 1))
    if ((failures >= max_poll_failures)); then
      echo "Lost contact with the MicroVM after $failures failed polls" >&2
      exit 1
    fi
    sleep $((poll_seconds * 2 ** failures))
    continue
  fi
  failures=0

  # The trailing x keeps command substitution from eating the chunk's final newlines.
  out=${out%x}
  read -r end full rc <<<"${out%%$'\n'*}"
  printf '%s' "${out#*$'\n'}"
  offset=$end

  if [[ $rc != - ]] && ((offset >= full)); then
    finished=true
    exit "$rc"
  fi
  if ((offset < full)); then
    continue
  fi
  sleep "$poll_seconds"
done
