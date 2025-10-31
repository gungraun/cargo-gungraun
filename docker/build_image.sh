#!/usr/bin/env bash

# spell-checker: ignore pnet filesyms oversion libcrypt ldconfig netcat lintian
# spell-checker: ignore armhf armmp loongson powerpcspe trixie octeon vmlinu
# spell-checker: ignore vmlinux

set -e

# Find a linux image for the given debian architecture
#
# Parameters:
#   $1: The debian architecture (amd64, i386)
debian_linux_image() {
  local debian_arch image

  debian_arch="${1:?The debian architecture should be present}"

  case "$debian_arch" in
  amd64)
    image="linux-image-amd64"
    ;;
  i386)
    image="linux-image-686"
    ;;
  armel)
    # or linux-image-rpi (raspberry pi)
    image="linux-image-marvell"
    ;;
  armhf)
    image="linux-image-armmp"
    ;;
  arm64)
    image="linux-image-arm64"
    ;;
  mips)
    # TODO: Unsupported since bullseye
    echo "mips is unsupported" >&2
    exit 1
    ;;
  mipsel)
    # or octeon, mips32r2el, 4kc-malta
    image="linux-image-loongson-3"
    ;;
  mips64el)
    # or octeon, mips64r2el, 5kc-malta
    image="linux-image-loongson-3"
    ;;
  powerpc)
    # TODO: Part of ports
    echo "powerpc is unsupported" >&2
    exit 1
    ;;
  powerpcspe)
    # TODO: DEAD
    echo "powerpcspe is unsupported" >&2
    exit 1
    ;;
  ppc64)
    # TODO: Part of ports
    echo "ppc64 is unsupported" >&2
    exit 1
    ;;
  ppc64el)
    image="linux-image-powerpc64le"
    ;;
  s390x)
    image="linux-image-s390x"
    ;;
  riscv64)
    # Requires trixie instead of bookwork
    image="linux-image-riscv64"
    ;;
  *)
    bail "Unable to find a linux image for debian architecture '$debian_arch'"
    ;;
  esac

  echo -n "$image"
}

################################################################################
## Prepare
################################################################################

source /lib.sh

if_trace set -x

install_temporary \
  cpio \
  gzip \
  kmod \
  libc-bin

debian_arch="$(debian_architecture "$CARGO_GUNGRAUN_TARGET")"
root_dir="${INTERNAL_KERNEL_BUILD_DIR:?A kernel build directory should be present}"
temp_dir="/qemu_temp"
qemu_dir="/qemu"

mkdir -p "$temp_dir"

# To be able to download the packages with apt-get into this directory without
# warnings
chmod 777 "$temp_dir"

mkdir -p "$root_dir"/{bin,boot,dev/pts,etc/dropbear,home,lib,lib64,mnt,proc,root,run/lock,sbin,sys,tmp,usr/{bin,sbin,lib,share},var/{log,tmp,run},modules,target}
mkdir -p "$qemu_dir"

cd "$temp_dir"

################################################################################
## Download and extract packages for the initrd
################################################################################

# If not already done
dpkg_architecture add "$debian_arch"

# We need the debug symbols of the libc6-dbg package available in the qemu image
# to be able to run Valgrind's memcheck.
download_packages_recursive \
  libcrypt1:"$debian_arch" \
  zlib1g:"$debian_arch" \
  libc6-dbg:"$debian_arch" \
  libc-bin:"$debian_arch" \
  busybox:"$debian_arch" \
  ncurses-term

download_packages \
  "$(debian_linux_image "$debian_arch")"

for dep in *; do
  dpkg -x "$dep" "$root_dir"
done

################################################################################
## Finish the kernel
################################################################################

# The kernel can be named vmlinux-* (e.g. powerpc64le) or vmlinuz-*
cp -v "${root_dir}/boot/vmlinu"* "${qemu_dir}/kernel"

################################################################################
## Extract and install the kernel modules for the image
################################################################################

mkdir -p "${temp_dir}/modules"

# Detect the kernel version and change into its directory
cd "$(find "${root_dir}/lib/modules" -maxdepth 1 -type d | tail -1)"
kernel_dir=$(pwd | grep -o 'lib/modules/.*')
kernel_version="$(basename "$kernel_dir")"

# These are the modules we basically need
base_modules="$(echo -e "virtio_net\n9p\n9pnet\n9pnet_virtio\nfailover\nnet_failover\n")"
base_modules+=$'\n'"$(find kernel/drivers/virtio -printf '%f\n' | xargs basename -s .ko)"

# These are all modules including their dependencies
modules=""
for mod in $base_modules; do
  modules+="$(find_dependencies "$mod" "$kernel_version" "$temp_dir")\n"
done
modules="$(echo -e "$modules" | sort -u)"

# Backup the modules with their dependencies keeping the directory structure
dest_dir="${temp_dir}/modules/${kernel_dir}"
mkdir -p "$dest_dir"
for mod in $modules; do
  cp -v --parents "$mod" "$dest_dir"
done

# These are the original depmod files of the linux-image-* package
find . -type f -maxdepth 1 -exec cp '{}' "${dest_dir}" \;

# Replace the original kernel modules with our reduced set of kernel modules
cd "$temp_dir"

rm -rf "${root_dir:?}/lib/modules"
mv "${temp_dir}/modules/lib/modules" "${root_dir}/lib"

# Regenerate the depmod files
depmod --verbose \
  --all \
  --basedir "$root_dir" \
  --filesyms "${root_dir}/boot/System.map-${kernel_version}" \
  "$kernel_version"

################################################################################
## Final steps and creation of basic files including the init script
################################################################################

# Cleanup most files we don't need in the qemu system image
rm -rf "${root_dir:?}/boot" \
  "${root_dir:?}/usr/share/doc" \
  "${root_dir:?}/usr/share/man" \
  "${root_dir:?}/usr/share/lintian" \
  "${root_dir:?}/usr/share/bug"

# Create the basic files and init script
cat <<'EOF' >"${root_dir}/etc/hosts"
127.0.0.1   localhost qemu
::1         localhost
EOF

cat <<'EOF' >"${root_dir}/etc/hostname"
qemu
EOF

cat <<'EOF' >"${root_dir}/etc/passwd"
root::0:0:root:/root:/bin/sh
EOF

touch "${root_dir}/var/log/lastlog"
dropbearkey -t rsa -f "${root_dir}/etc/dropbear/dropbear_rsa_host_key"
dropbearkey -t ecdsa -f "${root_dir}/etc/dropbear/dropbear_ecdsa_host_key"
dropbearkey -t ed25519 -f "${root_dir}/etc/dropbear/dropbear_ed25519_host_key"

host_ssh_dir=/root/.ssh
qemu_ssh_dir="${root_dir}/root/.ssh"
mkdir -p "$host_ssh_dir" "$qemu_ssh_dir"
chmod 0700 "$host_ssh_dir" "$qemu_ssh_dir"

dropbearkey -t rsa -f "${host_ssh_dir}/id_dropbear" |& grep '^ssh-' >"${qemu_ssh_dir}/authorized_keys"

cat <<'EOF' >"${root_dir}/init"
#!/bin/busybox sh

set -e

/bin/busybox --install

mkdir -p /dev /proc /run /sys /tmp

mount -t devtmpfs none /dev
mount -t proc proc /proc
mount -t sysfs sys /sys

mkdir -p /dev/pts
mount -t devpts none /dev/pts

mount -t tmpfs none /run
mkdir -p /run/lock

mount -t tmpfs none /tmp

mount

find /lib/modules -name '*.ko' -type f -print0 |
  xargs -0 -I {} basename -s .ko {} |
  while read -r mod; do modprobe "$mod"; done

ldconfig -v

ip addr add 127.0.0.1/8 dev lo
ip link set lo up

ip addr add 10.0.2.15/24 dev eth0
ip link set eth0 up

ip route add default via 10.0.2.2 dev eth0

while [[ -n "$1" ]]; do
  case "$1" in
  --mount)
    echo "$2" | {
      IFS=: read -r tag dir
      mkdir -p "$dir"
      mount -v -t 9p -o trans=virtio "$tag" "$dir"
    }
    shift 2
    ;;
  *)
    printf "init: Unrecognized argument: '%s'\n" "$1"
    exit 2
    ;;
  esac
done

exec dropbear -F -B
EOF

chmod +x "${root_dir}/init"

################################################################################
## Finish the initrd
################################################################################

cd "$root_dir"

# Finish the initrd
find . | cpio --create --format='newc' --quiet | gzip >"${qemu_dir}/initrd.gz"

################################################################################
## Cleanup
################################################################################

rm -rf "$root_dir" "$temp_dir"
dpkg_architecture remove "$debian_arch"

exit 0
