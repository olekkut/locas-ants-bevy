#!/usr/bin/env bash
# Build and launch the simulation in release mode (Linux / macOS).
set -euo pipefail
cd "$(dirname "$0")"

if [ "$(uname -s)" = "Linux" ]; then
    echo "Note: Bevy needs system packages on Linux (see README > Linux)."
fi

cargo run --release
