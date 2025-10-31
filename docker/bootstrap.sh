#!/usr/bin/env bash

set -e

for var in GUNGRAUN_RUNNER \
  GUNGRAUN_HOME \
  GUNGRAUN_VERSION \
  CARGO_TARGET_DIR \
  USER \
  HOME \
  SHELL; do
  [[ -n "${!var}" ]] || {
    echo "The $var variable should be set" >&2
    exit 1
  }
done

workspace=/workspace
target_dir="${CARGO_TARGET_DIR}"
host_target="$(rustc -vV | awk '/host:/ {print $2; exit}')"

# On windows there is no UID and GID and we apply the default
uid="${UID:-1000}"
gid="${GID:-1000}"

if [[ "$USER" != "root" ]]; then
  echo "${USER}::${uid}:${gid}::${HOME}:${SHELL}" >>/etc/passwd
  echo "${USER}::${gid}:${USER}" >>/etc/group
fi

mkdir -p "$HOME" "$workspace" "$target_dir" "$GUNGRAUN_HOME"
chown "${uid}:${gid}" "$HOME" "$workspace" "$target_dir" "$GUNGRAUN_HOME"

if [[ ! -e "$GUNGRAUN_RUNNER" ]]; then
  archive=gungraun-runner-v${GUNGRAUN_VERSION}-${host_target}
  cd /tmp
  wget "https://github.com/gungraun/gungraun/releases/download/v${GUNGRAUN_VERSION}/${archive}.tar.gz"
  tar xzf "${archive}.tar.gz" --strip-components=1 "${archive}/gungraun-runner"
  mv gungraun-runner "$GUNGRAUN_RUNNER"
  rm -rf "${archive}.tar.gz"
fi

echo "cargo-gungraun: bootstrap finished"
exec su "$USER" -c 'while true; do sleep 1; done'
