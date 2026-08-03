#!/usr/bin/env bash
set -euo pipefail

# Restore irqbalance when exiting for any reason.
trap 'sudo systemctl start irqbalance >/dev/null 2>&1 || true' EXIT

# Benchmark configuration
CPU_CORE=19
RT_PRIORITY=99

# Stop irqbalance to keep IRQ affinity stable during benchmarking.
sudo systemctl stop irqbalance >/dev/null 2>&1 || true

# Pin this shell to the benchmark CPU.
taskset -cp "$CPU_CORE" $$ >/dev/null

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

# Argument values to test
ARGS=(1.0 0.9 0.5)

RUNS_PER_ARG=3

echo "=============================================="
echo "Building all benchmark binaries..."
echo "=============================================="

RUSTFLAGS="-C target-cpu=native" cargo build --release --bins

for bin in "${BINARIES[@]}"; do
    BIN_PATH="./target/release/$bin"

    if [[ ! -x "$BIN_PATH" ]]; then
        echo "ERROR: $BIN_PATH not found or not executable. Skipping."
        continue
    fi

    echo
    echo "=============================================="
    echo "Benchmarking: $bin"
    echo "=============================================="

    for arg in "${ARGS[@]}"; do
        for run in $(seq 1 "$RUNS_PER_ARG"); do
            echo "----------------------------------------------"
            echo "Running: $bin arg=$arg (run $run/$RUNS_PER_ARG)"
            echo "----------------------------------------------"

            sudo taskset -c "$CPU_CORE" \
                chrt -f "$RT_PRIORITY" \
                "$BIN_PATH" "$arg"
        done
    done
done

echo
echo "=============================================="
echo "All benchmarks completed."
echo "=============================================="
