#!/bin/bash

set -e

apt update && apt install -y git
git config --global --add safe.directory "*"

## NOTE: Only change the following two lines
git config --global user.name "Your Name"
git config --global user.email "youremail@yourdomain.com"

TOP=$(git rev-parse --show-toplevel)
BIN=$TOP/bin  #"C:\Users\Sneha Gautam\rustos\bin\setup.sh"
DEP=$TOP/.dep
VER=nightly-2019-07-01
PROJ_PKG=(build-essential
     python3
     socat
     wget
     curl
     tar
     screen
     clang-8
     lld-8
     linux-image-extra-virtual)
QEMU_DEP=(libglib2.0-dev libpixman-1-dev zlib1g-dev)

# install pkgs
echo "[!] Installing packages"
export DEBIAN_FRONTEND=noninteractive 
export TZ=Etc/UTC
apt install -y ${PROJ_PKG[*]}
apt install -y ${QEMU_DEP[*]}

# install rustup
if ! [ -x "$(command -v rustup)" ]; then
    echo "[!] Installing rustup"

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

    export PATH=$HOME/.cargo/bin:$PATH
fi

rustup default $VER
rustup component add rust-src llvm-tools-preview clippy

# install cargo xbuild
mkdir -p $DEP
pushd $DEP
if ! [ -e cargo-xbuild ]; then
  git clone https://github.com/rust-osdev/cargo-xbuild
  pushd cargo-xbuild
  git checkout v0.5.20
  # https://github.com/rust-osdev/cargo-xbuild/pull/75
  git cherry-pick b24c849028eb7da2375288b1b8ab6a7538162bd7
  popd
fi
cargo install -f --path cargo-xbuild --locked
popd

# install cargo binutils
pushd $DEP
if ! [ -e cargo-objcopy ]; then
  git clone https://github.com/man9ourah/cargo-binutils.git
  cargo install -f --path cargo-binutils --locked
fi
popd

echo 'PATH="$HOME/.cargo/bin/:$PATH"' >>~/.bashrc

echo "[!] Setup complete"
echo "[!] Please run 'source ~/.bashrc' to update your environment"
