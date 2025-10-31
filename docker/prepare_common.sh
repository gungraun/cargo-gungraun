#!/usr/bin/env bash

set -e

source /lib.sh

if_trace set -x

printf "path-exclude=/usr/share/doc/*/examples/*\n" >>/etc/dpkg/dpkg.cfg.d/excludes

target="${CARGO_GUNGRAUN_TARGET:?A target should be present}"
toolchain_triple="$(debian_toolchain_triple "$target")"
debian_arch="$(debian_architecture "$target")"

dpkg_architecture add "$debian_arch"

case "$toolchain_triple" in
x86-64-linux-gnu) ;;
*) gfortran_cross=gfortran-"$toolchain_triple" ;;
esac

install_temporary \
  autoconf \
  automake \
  binutils \
  "binutils-$toolchain_triple" \
  ca-certificates \
  clang \
  cmake \
  curl \
  file \
  g++ \
  "g++-$toolchain_triple" \
  gcc \
  "gcc-${toolchain_triple}" \
  gfortran \
  "$gfortran_cross" \
  git \
  gzip \
  libc6-dev \
  "libc6-dev-${debian_arch}-cross" \
  libclang-dev \
  libtool \
  m4 \
  make \
  openssl \
  pkg-config \
  wget \
  bzip2 \
  xz-utils

exit 0
