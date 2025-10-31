#!/usr/bin/env bash

# spell-checker: ignore netdev hostfwd nographic fsdev maxmem minmem memuse
# spell-checker: ignore backgrounding numcpu logfile dbclient virt

print_message() {
  local format

  format="$1"
  shift

  # shellcheck disable=SC2059
  printf "${format}" "$@" >&2
}

set -e

source /lib.sh

lockfile=/run/lock/qemu.lock
logfile=/tmp/qemu.log
timeout=120
no_run=false
while [[ "$1" != "--" ]]; do
  case "$1" in
  --qemu-arch)
    qemu_arch="$(qemu_architecture "${2:?Missing argument for --qemu-arch}")"
    shift 2
    ;;
  --timeout)
    timeout=${2:?Missing argument for --timeout}
    shift 2
    ;;
  --log-file)
    logfile=${2:?Missing argument for --log-file}
    shift 2
    ;;
  --debug)
    GUNGRAUN_LOG=${2:?Missing argument for --debug}
    shift 2
    ;;
  --no-run)
    no_run=true
    shift
    ;;
  --envs)
    envs="${2:?Missing argument for --envs}"
    shift 2
    ;;
  --extra-args)
    extra_args="${2:?Missing argument for --extra-args}"
    shift 2
    ;;
  --accel)
    accel="${2:?Missing argument for --accel}"
    shift 2
    ;;
  *)
    printf "qemu_runner: Unrecognized argument: '%s'\n" "$1"
    exit 2
    ;;
  esac
done

shift

if_trace set -x

[[ -z "$qemu_arch" ]] && {
  echo "--qemu-arch is mandatory. Aborting..."
  exit 1
}

(
  flock -n 9 || exit 0

  machine=pc
  nic_model=virtio-net-pci

  case "$qemu_arch" in
  s390x)
    machine=s390-ccw-virtio
    nic_model=virtio-net-ccw
    ;;
  arm | aarch64)
    machine='virt'
    ;;
  *) ;;
  esac

  # 1G or if the total memory is smaller then the total
  minmem=$(awk '/MemTotal:/ {
    mem_total = $2 / (1024 * 1024);
    printf "%.1f", (mem_total < 1) ? mem_total : 1;
    exit;
  }' /proc/meminfo)

  # Available memory in gigabytes divided by 2 to keep some headroom
  mem_available=$(awk '/MemAvailable:/ {printf "%d", $2 / (1024 * 1024) / 2; exit}' /proc/meminfo)
  numcpu="$(nproc)"

  case "$qemu_arch" in
  mips*)
    memory=$(awk -v mem_available="$mem_available" -v minmem="$minmem" -v maxmem="2" 'BEGIN {
      result = (mem_available < minmem) ? minmem : mem_available;
      memory = (result <= maxmem) ? result : maxmem;
      printf "%.1fG\n", memory;
    }')
    ;;
  *)
    memory=$(awk -v mem_available="$mem_available" -v minmem="$minmem" 'BEGIN {
      result = (mem_available < minmem) ? minmem : mem_available;
      printf "%.1fG\n", result;
    }')
    ;;
  esac

  # These should already exist but for safety here again
  mkdir -p /target /workspace /gungraun_home

  # For testing: Setting -m to 100 causes a kernel panic with memory deadlock.
  # Setting -m to 100K causes it to error out because the initrd is too large.

  qemu_cmd=("qemu-system-${qemu_arch}"
    '-smp' "${numcpu}"
    '-m' "${memory}"
    '-kernel' '/qemu/kernel'
    '-initrd' '/qemu/initrd.gz'
    '-nic' "user,model=${nic_model},hostfwd=tcp::10022-:22,ipv6=off"
    '-nographic'
    '-monitor' 'none')

  append="console=ttyS0 --"
  for mount in "workspace:/workspace" "target:/target" "gungraun_home:/gungraun_home" "cargo:/root/.cargo" "rustup:/root/.rustup"; do
    IFS=: read -r tag path <<<"$mount"
    if [[ -e "$path" ]]; then
      qemu_cmd+=("-virtfs" "local,path=${path},security_model=passthrough,mount_tag=${tag}")
      append+=" --mount $mount"
    fi
  done
  qemu_cmd+=("-append" "$append")

  if [[ -n "$accel" ]]; then
    if grep -q kvm <<<"$accel"; then
      qemu_cmd+=('-enable-kvm')
    fi

    qemu_cmd+=('-machine' "${machine},accel=${accel}")
    accel_info="$accel"
  else
    accel_info="None"
  fi

  if [[ -n "$extra_args" ]]; then
    qemu_cmd+=("$extra_args")

    if cpu_info="$(echo "$extra_args" | grep -oP '(?<=-smp )\s*\S+')"; then
      cpu_info="$(tail -1 <<<"$cpu_info")"
    else
      cpu_info="$numcpu"
    fi
    if mem_info="$(echo "$extra_args" | grep -oP '(?<=-m )\s*\S+')"; then
      mem_info="$(tail -1 <<<"$mem_info")"
    else
      mem_info="$memory"
    fi

    arg_info="$extra_args"
  else
    cpu_info="$numcpu"
    mem_info="$memory"
    arg_info=None
  fi

  if_debug print_message "Qemu Command: %s\n" "${qemu_cmd[*]}"

  touch "$logfile"
  print_message "Starting qemu:\nArchitecture: %s\nCPU(s): %s\nMemory: %s\nAccelerator: %s\nExtra args: %s\n" "$qemu_arch" "$cpu_info" "$mem_info" "$accel_info" "$arg_info"

  if is_debug; then
    ("${qemu_cmd[@]}" |& tee "$logfile") &
  else
    "${qemu_cmd[@]}" >"$logfile" 2>&1 &
  fi
  pid=$!

  # Wait for qemu to print dropbear's final 'Not backgrounding' message or until
  # the timeout is reached
  seconds=0
  while true; do
    if ! kill -0 "$pid" 2>/dev/null; then
      if_not_debug print_message "\nStarting qemu failed:\n"
      if_not_debug tail -1 "$logfile"
      if_not_debug print_message "\nSee the logfile '${logfile}' for more info"

      exit 1
    elif tail -10 "$logfile" | grep -q -e '---\[ end Kernel panic'; then
      kill -9 $pid

      if_not_debug print_message "\nStarting qemu failed:\n"
      if_not_debug tail -1 "$logfile"
      if_not_debug print_message "\nSee the logfile '${logfile}' for more info"

      exit 1
    elif tail -10 "$logfile" | grep -q "Not backgrounding"; then
      break
    elif ((seconds >= timeout)); then
      kill -9 $pid

      print_message "\nStarting qemu failed:\nTimeout of %s seconds reached. Exiting.\n" "$timeout"
      if_not_debug print_message "\nSee the logfile '${logfile}' for more info"

      exit 1
    elif ((seconds != 0 && seconds % 60 == 0)); then
      if_not_debug print_message '\n.'
    else
      if_not_debug print_message '.'
    fi

    sleep 1
    _=$((seconds++))
  done

  print_message "\nQemu startup complete\n"

  # A quick check that the connection is ready. Furthermore it's important to
  # accept the key here automatically or else `scp` will try to interactively
  # ask if we want to accept the key. Unlike `dbclient`, `scp` doesn't have the
  # `-y` flag.
  dbclient -q -p 10022 -y root@localhost 'echo Connection succeeded' | grep -q 'Connection succeeded'
  # TODO: check if this is necessary with the current method
  if [[ -e '/usr/bin/gungraun-runner' ]]; then
    scp -qpP 10022 /usr/bin/gungraun-runner root@localhost:/usr/bin/
  elif [[ -e '/usr/bin/iai-callgrind-runner' ]]; then
    scp -qpP 10022 /usr/bin/iai-callgrind-runner root@localhost:/usr/bin/
  fi

) 9>"$lockfile"

if ! $no_run; then
  if [[ -t 1 ]] && [[ -t 2 ]]; then
    tty_flag='-t'
  else
    tty_flag='-T'
  fi

  exec dbclient "${tty_flag}" -q -p 10022 -y root@localhost "cd $(pwd); ${envs} ${*}"
fi
