#!/usr/bin/env bash
# Runs a command in a checkout of one commit on a MicroVM and streams its output.
#
# By default each run gets its own MicroVM, restored from the newest checkpoint of the
# template and deleted afterwards. With --checkpoint the command runs on the template
# itself, which is then checkpointed so later runs start from its warm Bazel state.
#
# Usage: runner/vm-run.sh [--registry-push] [--checkpoint] <template-name-or-id> <commit-sha> -- <command...>
#   e.g. runner/vm-run.sh pollingjuegos-runner "$sha" -- bazel test //...
#
# Environment:
#   DIGITALOCEAN_ACCESS_TOKEN  token for doctl (DIGITALOCEAN_TOKEN is used as a fallback)
#   BUILDBUDDY_API_KEY         optional; handed to Bazel through a credential helper
#   VM_GITHUB_TOKEN            optional; exported to the command as GH_TOKEN
#   VM_RUN_REPO_URL            optional; defaults to this repository on GitHub
set -euo pipefail

usage() {
  echo "usage: $0 [--registry-push] [--checkpoint] <template-name-or-id> <commit-sha> -- <command...>" >&2
  exit 2
}

registry_push=false
checkpoint=false
while [[ ${1:-} == --* ]]; do
  case $1 in
  --registry-push) registry_push=true ;;
  --checkpoint) checkpoint=true ;;
  *) usage ;;
  esac
  shift
done
(($# >= 4)) && [[ $3 == -- ]] || usage
template=$1
sha=$2
shift 3
[[ $sha =~ ^[0-9a-f]{40}$ ]] || {
  echo "commit must be a full 40-character SHA" >&2
  exit 2
}

repo_url=${VM_RUN_REPO_URL:-https://github.com/ryansheppard/pollingjuegos}
poll_seconds=10
# exec output is capped at 4 MiB, so each poll fetches at most this much log.
chunk_bytes=$((3 * 1024 * 1024))
max_poll_failures=6
# More than one, so a run that picked a checkpoint just before a refresh can still use it.
keep_checkpoints=2
job_tag=pollingjuegos-job

# doctl reads DIGITALOCEAN_ACCESS_TOKEN itself, which keeps the token off its command line.
if [[ -z ${DIGITALOCEAN_ACCESS_TOKEN:-} && -n ${DIGITALOCEAN_TOKEN:-} ]]; then
  export DIGITALOCEAN_ACCESS_TOKEN=$DIGITALOCEAN_TOKEN
fi

if [[ ! $template =~ ^[0-9a-f]{8}-[0-9a-f]{4}- ]]; then
  name=$template
  template=$(timeout 30 doctl compute microvm list --format ID,Name --no-header | awk -v n="$name" '$2 == n { print $1 }')
  [[ -n $template ]] || {
    echo "No MicroVM named $name" >&2
    exit 1
  }
fi

run_id=${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-0}-$$-$(date +%s)
dir=/tmp/vm-run/$run_id
vm=

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

vm_state() { timeout 30 doctl compute microvm get "$vm" --format State --no-header | tr -d '[:space:]'; }

wait_running() {
  for _ in {1..24}; do
    [[ $(vm_state) == running ]] && return
    sleep 5
  done
  echo "The MicroVM did not reach running (state: $(vm_state))" >&2
  exit 1
}

job_name=$job_tag-$run_id
finished=false
cleanup() {
  # A second Ctrl-C during cleanup should abandon it, not restart it.
  trap - INT TERM
  if [[ $checkpoint == true ]]; then
    if [[ $finished != true ]]; then
      echo "Stopping the remote job" >&2
      remote "test -f $dir/pid && kill -TERM -- -\$(cat $dir/pid)" || true
    fi
    remote "rm -rf $dir" || true
    # One attempt only: the idle timer pauses it anyway if this fails.
    echo "Pausing the template MicroVM"
    timeout 30 doctl --http-retry-max 0 compute microvm pause "$vm" >/dev/null || echo "Pausing failed; the idle timer will pause it" >&2
  else
    # Looked up by name so a VM is still found when create was interrupted, or when a
    # retried create made a second one.
    local ids
    ids=$(timeout 30 doctl compute microvm list --name "$job_name" --format ID --no-header) || ids=$vm
    read -ra ids <<<"$ids"
    if ((${#ids[@]})); then
      echo "Deleting the job MicroVM"
      timeout 60 doctl compute microvm delete --force "${ids[@]}" >/dev/null ||
        echo "Deleting $job_name failed; delete it by hand" >&2
    fi
  fi
}

trap cleanup EXIT
# Bash skips EXIT traps when killed by these, and Actions sends them on cancel.
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ $checkpoint == true ]]; then
  vm=$template
  # exec does not wake a paused MicroVM; it hangs until the API's proxy times out.
  if [[ $(vm_state) != running ]]; then
    echo "Resuming the template MicroVM"
    timeout 60 doctl compute microvm resume "$vm" >/dev/null
    wait_running
  fi
else
  ckpt=$(timeout 30 doctl compute microvm checkpoint list --microvm-id "$template" |
    awk '$1 ~ /^[0-9a-f]{8}-/ && /CHECKPOINT_AVAILABLE/ { print $1; exit }')
  [[ -n $ckpt ]] || {
    echo "The template has no available checkpoint; run with --checkpoint first" >&2
    exit 1
  }
  echo "Creating $job_name from checkpoint $ckpt"
  vm=$(timeout 120 doctl compute microvm create "$job_name" --checkpoint-id "$ckpt" \
    --tag "$job_tag" --auto-resume=false | awk '$1 ~ /^[0-9a-f]{8}-/ { print $1; exit }')
  [[ -n $vm ]] || {
    echo "Creating $job_name returned no ID" >&2
    exit 1
  }
  wait_running
fi

remote "umask 077 && mkdir -p $dir"
if [[ -n ${BUILDBUDDY_API_KEY:-} ]]; then
  put_secret "$dir/buildbuddy-api-key" "$BUILDBUDDY_API_KEY"
fi
if [[ -n ${VM_GITHUB_TOKEN:-} ]]; then
  put_secret "$dir/github-token" "$VM_GITHUB_TOKEN"
fi
if [[ $registry_push == true ]]; then
  registry_config=$(doctl registry docker-config --read-write --expiry-seconds 1800)
  put_secret "$dir/docker/config.json" "$registry_config"
fi

job=$(base64 -w0 <"$(dirname "${BASH_SOURCE[0]}")/job.sh")
args=$(printf '%q ' "$dir" "$repo_url" "$sha" "$@")
remote "printf %s $job | base64 -d > $dir/job.sh && setsid nohup bash $dir/job.sh $args > $dir/log 2>&1 < /dev/null &"

capture_checkpoint() {
  local new status='' old
  # The job dir is gone before capture so no log or leftover secret lands in the checkpoint.
  remote "rm -rf $dir"
  # The checkpoint commands ignore --format and --no-header, and an empty name column
  # shifts the fields, so rows are matched by their leading UUID and status text.
  new=$(timeout 60 doctl compute microvm checkpoint create "$vm" --name "${sha::12}-$run_id" |
    awk '$1 ~ /^[0-9a-f]{8}-/ { print $1; exit }')
  [[ -n $new ]] || {
    echo "Creating a checkpoint returned no ID" >&2
    exit 1
  }
  echo "Capturing checkpoint $new"
  for _ in {1..120}; do
    status=$(timeout 30 doctl compute microvm checkpoint get "$new" | grep -oE -m1 'CHECKPOINT_[A-Z_]+' || true)
    [[ $status == CHECKPOINT_AVAILABLE || $status =~ FAIL|ERROR ]] && break
    sleep 5
  done
  [[ $status == CHECKPOINT_AVAILABLE ]] || {
    echo "Checkpoint $new did not become available (status: $status)" >&2
    exit 1
  }
  mapfile -t old < <(timeout 30 doctl compute microvm checkpoint list --microvm-id "$vm" |
    awk -v keep="$keep_checkpoints" '$1 ~ /^[0-9a-f]{8}-/ && /CHECKPOINT_AVAILABLE/ && ++n > keep { print $1 }')
  if ((${#old[@]})); then
    echo "Deleting ${#old[@]} old checkpoint(s)"
    timeout 60 doctl compute microvm checkpoint delete --force "${old[@]}" >/dev/null ||
      echo "Deleting old checkpoints failed" >&2
  fi
}

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
    if [[ $checkpoint == true && $rc == 0 ]]; then
      capture_checkpoint
    fi
    exit "$rc"
  fi
  if ((offset < full)); then
    continue
  fi
  sleep "$poll_seconds"
done
