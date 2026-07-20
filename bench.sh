#!/usr/bin/env bash
set -euo pipefail

# All binaries to build & benchmark
BINARIES=(
    get_hb_48
    get_hb_75
    get_hb_87
    gethb_jumbo_48
    gethb_jumbo_75
    gethb_jumbo_87
    get_vegosh_48
    get_vegosh_75
    get_vegosh_87
    getv_jumbo_48
    getv_jumbo_75
    getv_jumbo_87
)

# Argument values to test, each run 3x sequentially, in this order
ARGS=(1.0 0.9 0.5)

RUNS_PER_ARG=3
CPU_CORE=2
RT_PRIORITY=99

for bin in "${BINARIES[@]}"; do
    echo "=============================================="
    echo "Building: $bin"
    echo "=============================================="
    RUSTFLAGS="-C target-cpu=native" cargo build --release --bin "$bin"

    BIN_PATH="./target/release/$bin"

    if [[ ! -x "$BIN_PATH" ]]; then
        echo "ERROR: $BIN_PATH not found or not executable after build. Skipping."
        continue
    fi

    for arg in "${ARGS[@]}"; do
        for run in $(seq 1 "$RUNS_PER_ARG"); do
            echo "----------------------------------------------"
            echo "Running: $bin arg=$arg (run $run/$RUNS_PER_ARG)"
            echo "----------------------------------------------"
            sudo taskset -c "$CPU_CORE" chrt -f "$RT_PRIORITY" "$BIN_PATH" "$arg"
        done
    done
done

echo "All binaries built and benchmarked."
