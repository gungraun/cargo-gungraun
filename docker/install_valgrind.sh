#!/usr/bin/env bash

# spell-checker: ignore lbzip

set -e

source /lib.sh

if_trace set -x

version="${VALGRIND_VERSION:?A valgrind version should be present}"
destdir="${1:?A destination directory for valgrind should be present}"
valgrind_build_dir="${HOME}/valgrind"
toolchain=$(valgrind_toolchain_triple "${CARGO_GUNGRAUN_TARGET}")

install_temporary lbzip2

mkdir "$valgrind_build_dir"
cd "$valgrind_build_dir"
wget https://sourceware.org/pub/valgrind/valgrind-"${version}".tar.bz2
tar xf valgrind-"${version}".tar.bz2

export CC="${toolchain}-gcc"
export LD="${toolchain}-ld"
export AR="${toolchain}-ar"

which "$CC" "$LD" "$AR"

cd valgrind-"${version}"

./autogen.sh

# According to valgrind/configure file, the CARGO_GUNGRAUN_TARGET is mostly
# supported as is for the --host variable. If the target is not supported by
# valgrind, configure will exit with an error.
./configure --prefix="/usr" \
  --host="${toolchain}"

make -j"$(nproc)"
# DESTDIR is just the path where we install valgrind temporarily. The --prefix
# above is the important path, and where valgrind assumes it is installed.
make -j"$(nproc)" install DESTDIR="$destdir"

cd
rm -rf "$valgrind_build_dir"

exit 0
