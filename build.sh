#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "======================================="
echo "Building ztrim Release Binary (Linux)..."
echo "======================================="

cargo build --release

if [ -f "target/release/ztrim" ]; then
    cp "target/release/ztrim" "ztrim"
    chmod +x "ztrim"
    echo ""
    echo "[SUCCESS] ztrim built at target/release/ztrim (copied to ./ztrim)"
    echo ""
else
    echo ""
    echo "[ERROR] target/release/ztrim not found!"
    exit 1
fi
