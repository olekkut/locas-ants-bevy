#!/bin/bash

# Terminate any running instances of optimistic-carson
echo "Stopping any running instances of optimistic-carson..."
if pgrep -x "optimistic-carson" >/dev/null; then
    pkill -x "optimistic-carson"
fi

# Detect Operating System
OS="$(uname -s)"
echo "Detected OS: $OS"

if [ "$OS" = "Linux" ]; then
    echo "--------------------------------------------------------"
    echo "Linux Build Notice:"
    echo "Please ensure Bevy's development dependencies are installed."
    echo "Ubuntu/Debian: sudo apt-get install pkg-config libx11-dev libasound2-dev libudev-dev"
    echo "Fedora: sudo dnf install pkgconf-pkg-config libX11-devel alsa-lib-devel systemd-devel"
    echo "Arch: sudo pacman -S pkgconf libx11 alsa-lib udev"
    echo "--------------------------------------------------------"
    echo ""
fi

echo "Compiling the application..."
cargo build

echo "Deploying assets to target directory..."
mkdir -p target/debug
cp -r assets target/debug/

echo "Running the application..."
cargo run
