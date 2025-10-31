#!/usr/bin/env bash

set -e

source /lib.sh

if_trace set -x

bin="${1:?The executable should be present}"
shift

triple="${CARGO_GUNGRAUN_TARGET:?The build target triple should be present}"
runner="${GUNGRAUN_RUNNER:?The GUNGRAUN_RUNNER variable should be set}"
qemu_arch="$(qemu_architecture "${triple}")"

if version="$("$runner" --version)"; then
  version="$(cut -d' ' -f2 - <<<"$version")"
else
  version=0.1.0
fi

args=("--qemu-arch" "${triple}")
if is_trace; then
  args+=('--debug' 'trace')
elif is_debug; then
  args+=('--debug' 'debug')
fi

if [[ -n "$CARGO_GUNGRAUN_QEMU_TIMEOUT" ]]; then
  args+=('--timeout' "$CARGO_GUNGRAUN_QEMU_TIMEOUT")
fi
if [[ -n "$CARGO_GUNGRAUN_QEMU_EXTRA_ARGS" ]]; then
  args+=('--extra-args' "$CARGO_GUNGRAUN_QEMU_EXTRA_ARGS")
fi
if [[ -n "$CARGO_GUNGRAUN_QEMU_ACCELERATOR" ]]; then
  args+=('--accel' "$CARGO_GUNGRAUN_QEMU_ACCELERATOR")
fi

qemu_cmd=("qemu-$qemu_arch")
qemu_runner_cmd=('/qemu_runner.sh')
qemu_runner_cmd+=("${args[@]}")

if "${qemu_cmd[@]}" "$bin" --gungraun-run invalid |& grep -q "function.*invalid.*not found in this scope"; then
  if printf '0.17.1\n%s' "$version" | sort -V | head -1 | grep -q '0\.17\.1'; then
    # Start qemu before running the benchmark to support running multiple
    # benchmarks in parallel if the benchmark harness allows it
    "${qemu_runner_cmd[@]}" --no-run --
    exec "${qemu_cmd[@]}" "$bin" "$@"
  else
    echo "runner.sh: Error: cargo-gungraun needs a gungraun version >= 0.17.1" >&2
    exit 1
  fi
elif "${qemu_cmd[@]}" "$bin" --iai-run invalid |& grep -q "function.*invalid.*not found in this scope"; then
  echo "runner.sh: Error: iai-callgrind is not supported by cargo-gungraun. Please update to gungraun >= 0.17.1" >&2
  exit 1
else
  "${qemu_runner_cmd[@]}" --no-run --
  exec "${qemu_runner_cmd[@]}" -- "$bin" "$@"
fi
