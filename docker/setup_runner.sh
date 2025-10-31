#!/usr/bin/env bash

# spell-checker: ignore libxen

set -e

source /lib.sh

if_trace set -x

printf "path-exclude=/usr/share/doc/*/examples/*\n" >>/etc/dpkg/dpkg.cfg.d/excludes

target="${CARGO_GUNGRAUN_TARGET:?A target should be present}"
toolchain_triple="$(debian_toolchain_triple "$target")"
debian_arch="$(debian_architecture "$target")"

dpkg_architecture add "$debian_arch"

cd /usr/lib/
ln -s /usr/lib64/libslirp.so.0.4.0 .
ln -s libslirp.so.0.4.0 libslirp.so.0
ln -s libslirp.so.0 libslirp.so

# The dynamically linked qemu and libslirp requires most of these packages. See
# the install-qemu.sh for more details.
#
# others:
# * clang, libclang-dev (required by bindgen) and
#   g++, gcc to be able to build gungraun with client requests
# * util-linux for `setarch`
# * wget, ca-certificates, tar and gzip to be able to download and unpack
#   gungraun-runner
install_packages \
  ca-certificates \
  clang \
  fuse3 \
  g++ \
  "g++-$toolchain_triple" \
  gcc \
  "gcc-${toolchain_triple}" \
  gzip \
  libblkid1 \
  libbz2-1.0 \
  libc6 \
  libc6-dev \
  "libc6-dev-${debian_arch}-cross" \
  libclang-dev \
  libffi8 \
  libglib2.0 \
  libglib2.0-dev \
  libmount1 \
  libpcre2-8-0 \
  libssh-4 \
  libxen-dev \
  libzstd1 \
  pkg-config \
  tar \
  util-linux \
  wget \
  zlib1g

install_temporary \
  libc-bin

if_debug ldconfig -v
if_not_debug ldconfig

if_debug ldconfig -p

exit 0
