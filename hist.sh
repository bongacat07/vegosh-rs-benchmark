#!/usr/bin/env bash
set -euo pipefail

# Build only the histogram binaries
for file in src/bin/insert*_hist*.rs; do
    bin=$(basename "$file" .rs)
    cargo build --release --bin "$bin"
done

# Run only the histogram binaries
for file in src/bin/insert*_hist*.rs; do
    bin=$(basename "$file" .rs)
    echo "Running $bin..."
    cargo run --release --bin "$bin"
done

mkdir -p hist
mv *.csv hist/
