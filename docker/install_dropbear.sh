#!/usr/bin/env bash

# spell-checker: ignore localoptions dbclient ddropbear cflags

set -e

source /lib.sh

if_trace set -x

version="${DROPBEAR_VERSION:?A dropbear version should be present}"
dest_dir="${INTERNAL_KERNEL_BUILD_DIR:?The kernel build dir should be present}"
toolchain=$(debian_toolchain_triple "${CARGO_GUNGRAUN_TARGET}")
tool_prefix=$(debian_tool_prefix "${CARGO_GUNGRAUN_TARGET}")
debian_arch="$(debian_architecture "${CARGO_GUNGRAUN_TARGET}")"

# To be sure since we install dropbear statically on the host
apt-get purge -y dropbear

install_packages \
  zlib1g

install_temporary \
  zlib1g:"$debian_arch" \
  zlib1g-dev \
  zlib1g-dev:"$debian_arch"

build_dir="${HOME}/dropbear"
mkdir -p "$build_dir"
pushd "$build_dir"

wget "https://github.com/mkj/dropbear/archive/refs/tags/DROPBEAR_${version}.tar.gz"
tar xzf "DROPBEAR_${version}.tar.gz"
cd "dropbear-DROPBEAR_${version}"

# Two builds. The first is for the host and the second for the image. The image
# build needs to be cross-compiled with the target triple.

# https://github.com/mkj/dropbear/blob/master/src/default_options.h
cp /dropbear_options.h localoptions.h
common_opts=("--prefix=/usr" "--enable-static" "--disable-syslog")

./configure "${common_opts[@]}"

make -j"$(nproc)" PROGRAMS="dbclient dropbearkey scp"
make PROGRAMS="dbclient dropbearkey scp" install

make clean

export CC="${tool_prefix}-gcc"
export LD="${tool_prefix}-ld"
export AR="${tool_prefix}-ar"

which "$CC" "$LD" "$AR"

./configure "${common_opts[@]}" \
  --host="$toolchain"

make -j"$(nproc)" PROGRAMS="dropbear scp"
make DESTDIR="$dest_dir" PROGRAMS="dropbear scp" install

popd
rm -rf "$build_dir"

exit 0
