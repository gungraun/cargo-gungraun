#!/usr/bin/env bash

# spell-checker: ignore gnueabi gnueabihf armhf gnuspe powerpcspe subshell
# spell-checker: ignore thumbv

# Return with error exit and print the given message to stderr
#
# Parameters:
#   $1: The error message
bail() {
  local msg filename
  msg="${*}"
  filename="$(basename "${BASH_SOURCE[1]}")"

  echo "${filename}: ${msg}" >&2
  exit 1
}

# Return the rust target triple of this host
host_target() {
  rustc -vV | awk '/host:/ {print $2; exit}'
}

# Return true if the log level is trace
is_trace() {
  [[ "${GUNGRAUN_LOG,,}" == "trace" ]]
}

# Return true if the log level is debug
is_debug() {
  [[ "${GUNGRAUN_LOG,,}" == "debug" ]] || is_trace
}

# If debugging is activated, execute the given arguments in a subshell
if_debug() {
  if is_debug; then
    "$@"
  else
    true
  fi
}

# If debugging is set to trace, execute the given arguments in a subshell
if_trace() {
  if is_trace; then
    "$@"
  else
    true
  fi
}

# If debugging is not set, execute the given arguments in a subshell
if_not_debug() {
  if ! is_debug; then
    "$@"
  else
    true
  fi
}

# Return true if the docker build rust triple equals the host triple
is_native() {
  local triple host_target

  triple="${CARGO_GUNGRAUN_TARGET:?The target triple should be present}"
  host_target="$(host_target)"

  [[ "$triple" == "$host_target" ]]
}

# Install all given packages as automatically installed
#
# Marking as auto excludes packages which are already marked as manually
# installed.
#
# Parameters:
#   $@: the space separated list of packages
install_temporary() {
  local packages mark_auto

  packages=("${@}")
  mark_auto="$(
    comm -31 <(apt-mark showmanual | sort) <(printf "%s\n" "${packages[@]}" | sort -u) |
      tr '\n' ' ' |
      awk '{$1=$1; print}'
  )"
  install_packages "${packages[@]}"
  # shellcheck disable=2086
  [[ -n "$mark_auto" ]] && apt-mark auto $mark_auto || true
}

# Install all given packages as manually installed
#
# Parameters:
#   $@: The space separated list of packages
install_packages() {
  apt-get update && apt-get install --assume-yes --no-install-recommends "${@}"
}

# Internal worker function
_download_package() {
  local package filename depends multi_depends all recursive

  package="$1"
  filename="$2"
  recursive=${3:-true}

  all=""

  depends="$(apt-cache depends -i "$package")"
  multi_depends="$(echo "$depends" | awk '/Depends: <.*>/ {getline; print $1}')"
  if [[ -n "$multi_depends" ]]; then
    all+="$multi_depends\n"
  fi

  all+="$(echo -e "$depends" | grep 'Depends: [^<]' || true)"
  all="$(echo -e "$all" | sed -e 's/[<>|]//g' -e 's/PreDepends://g' -e 's/Depends://g' -e 's/ //g')"

  for dep in $all; do
    if ! grep "$dep" "$filename"; then
      echo "$dep" >>"$filename"
      if $recursive; then
        _download_package "$dep" "$filename" "$recursive"
      fi
    fi
  done

  echo "$package" >>"$filename"
}

# Internal worker function
_download_packages() {
  for package in "${@}"; do
    filename=deps
    touch "$filename"

    _download_package "$package" "$filename" "$recursive"

    while read -r dep; do
      apt-get download "$dep"
    done <"$filename"

    apt-get download "$package"
    rm "$filename"
  done
}

# Downloads the given packages with all dependencies recursively
#
# Parameters:
#   $@: Space separated list of packages
download_packages_recursive() {
  recursive=true

  _download_packages "$@"

  unset recursive
}

# Downloads the given packages including their direct dependencies
#
# Parameters:
#   $@: Space separated list of packages
download_packages() {
  recursive=false

  _download_packages "$@"

  unset recursive
}

# Add or remove a dpkg architecture
#
# Parameters:
#   $1: the action; Can be either `add` or `remove`
#   $2: The foreign architecture to set or unset
dpkg_architecture() {
  local action foreign_arch native_arch
  action="${1:?The action should be present. Valid actions are 'add' and 'remove'}"
  foreign_arch="${2:?The foreign architecture should be present}"

  native_arch="$(dpkg --print-architecture)"
  if [[ "$foreign_arch" != "$native_arch" ]]; then
    case "$action" in
    add)
      dpkg --add-architecture "$foreign_arch"
      ;;
    remove)
      dpkg --remove-architecture "$foreign_arch"
      ;;
    *)
      bail "Invalid action for dpkg_architecture: ${action}"
      ;;
    esac

    apt-get update
  fi
}

# Convert the given rust triple to the triple used in the package names of gnu
# toolchains (like gcc-i686-linux-gnu)
#
# This list is tailored to our list of supported targets and is not universally
# correct. For example a rust target with arch-vendor-os or arch-vendor-os-env
# is expected but this is not true for all triples.
debian_toolchain_triple() {
  local rust_triple host_cpu host_os triple

  rust_triple="${1:?The rust triple should be present}"

  IFS='-' read -r host_cpu _ host_os <<<"$rust_triple"

  case "${host_cpu}" in
  i?86) host_cpu=i686 ;;
  amd64 | x86_64) host_cpu=x86-64 ;;
  powerpc*) ;;
  s390x) ;;
  arm* | thumbv*) host_cpu=arm ;;
  aarch64*) host_cpu=aarch64 ;;
  mips*) ;;
  nanomips) ;;
  riscv64*) host_cpu=riscv64 ;;
  *)
    bail "Unsupported rust triple '${rust_triple}'"
    ;;
  esac

  echo -n "${host_cpu}-${host_os}"
}

# Extract the debian architecture from the rust target triple
#
# Parameters:
#   $1: The rust triple (x86_64-unknown-linux-gnu)
debian_architecture() {
  local rust_triple host_cpu host_os triple

  rust_triple="${1:?The rust triple should be present}"

  case "$rust_triple" in
  x86_64-unknown-linux-gnu)
    arch="amd64"
    ;;
  i?86-unknown-linux-gnu)
    arch="i386"
    ;;
  arm-unknown-linux-gnueabi | armv7-unknown-linux-gnueabi)
    arch="armel"
    ;;
  arm-unknown-linux-gnueabihf | armv7-unknown-linux-gnueabihf)
    arch="armhf"
    ;;
  aarch64-unknown-linux-gnu)
    arch="arm64"
    ;;
  mips-unknown-linux-gnu)
    arch="mips"
    ;;
  mipsel-unknown-linux-gnu)
    arch="mipsel"
    ;;
  mips64el-unknown-linux-gnu)
    arch="mips64el"
    ;;
  powerpc-unknown-linux-gnu)
    arch="powerpc"
    ;;
  powerpc-unknown-linux-gnuspe)
    arch="powerpcspe"
    ;;
  powerpc64-unknown-linux-gnu)
    arch="ppc64"
    ;;
  powerpc64le-unknown-linux-gnu)
    arch="ppc64el"
    ;;
  s390x-unknown-linux-gnu)
    arch="s390x"
    ;;
  riscv64gc-unknown-linux-gnu)
    arch="riscv64"
    ;;
  *)
    bail "Unable to convert rust triple '$rust_triple' to a debian architecture"
    ;;
  esac

  echo -n "$arch"
}

# Extract the qemu architecture from the rust target triple
#
# The list of possible qemu architectures isn't exhaustive. Just the
# architectures valgrind supports are converted
#
# Parameters:
#   $1: the rust triple (x86_64-unknown-linux-gnu)
qemu_architecture() {
  local rust_triple arch

  rust_triple="${1:?The rust triple should be present}"
  IFS='-' read -r host_cpu _ <<<"$rust_triple"

  case "$host_cpu" in
  x86_64)
    arch="x86_64"
    ;;
  i?86)
    arch="i386"
    ;;
  arm | armv7)
    arch="arm"
    ;;
  aarch64)
    arch="aarch64"
    ;;
  mips)
    arch="mips"
    ;;
  mipsel)
    arch="mipsel"
    ;;
  mips64el)
    arch="mips64el"
    ;;
  powerpc)
    arch="ppc"
    ;;
  # This is only correct for softmmu targets. user targets distinguish between
  # little and big endian.
  powerpc64 | powerpc64le)
    arch="ppc64"
    ;;
  s390x)
    arch="s390x"
    ;;
  riscv64 | riscv64gc)
    arch="riscv64"
    ;;
  *)
    bail "Unable to convert rust triple \
      '$rust_triple' to a supported qemu architecture"
    ;;
  esac

  echo -n "$arch"
}

# Extracts the valgrind toolchain triple from the rust target triple
#
# Parameters:
#   $1 - The rust target triple (x86_64-unknown-linux-gnu)
#
# Errors:
#   If the rust target triple is in an invalid format
valgrind_toolchain_triple() {
  local rust_triple host_cpu host_os triple

  rust_triple="${1:?The rust triple should be present}"

  IFS='-' read -r host_cpu _ host_os <<<"$rust_triple"

  # Mostly from valgrind repository configure script
  case "${host_cpu}" in
  i?86) ;;
  x86_64 | amd64) ;;
  powerpc64) ;;
  powerpc64le) ;;
  powerpc) ;;
  s390x) ;;
  armv8*) ;;
  armv7*) ;;
  arm*) ;;
  aarch64*) ;;
  mips) ;;
  mipsel) ;;
  mips64*) ;;
  mipsisa64*) ;;
  nanomips) ;;
  riscv64) ;;
  riscv64gc) host_cpu=riscv64 ;; # This spec is not recognized by valgrind
  *)
    bail "Invalid host cpu spec: '${host_cpu}' in '${rust_triple}'"
    ;;
  esac

  triple=${host_cpu}-${host_os}
  case $triple in
  *-*-*) echo -n "$triple" ;;
  *)
    bail "Invalid target specification: '${rust_triple}'"
    ;;
  esac
}

# Finds kernel module dependencies recursively
#
# Prints all kernel modules in the format `kernel/path/to/module.ko` including
# the starting kernel module. For this function to work correctly the working
# directory has to be `$base_dir/lib/modules/$kernel`.
#
# Note that this method does not check for duplicates. `modinfo` has to be
# installed and available.
#
# Parameters:
#   $1: The name of a module (without .ko suffix or path segments)
#   $2: The kernel version
#   $3: Optionally a base directory. The default is '/'
find_dependencies() {
  local module seen deps kernel

  module="${1:?A module name should be present}"
  kernel="${2:?The kernel version should be present}"
  base_dir="${3:-/}"
  seen="$4"

  # Check if the module has already been seen to avoid infinite loops
  if [[ "$seen" == *"$module"* ]]; then
    return
  fi

  # Add the module to the seen list
  seen+="$module "
  module_path="$(find . -iname "${module}.ko" | grep -o 'kernel/.*')"

  # Get the dependencies of the module
  deps=$(modinfo -b "$base_dir" -k "$kernel" -F depends "$module_path" | tr ',' ' ')

  # Print the module
  echo "$module_path"

  # Recursively find dependencies for each dependency
  for dep in $deps; do
    find_dependencies "$dep" "$kernel" "$base_dir" "$seen"
  done
}
