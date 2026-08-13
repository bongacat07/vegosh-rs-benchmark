#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# Benchmark configuration
# ============================================================

CPU_CORE=19
FIXED_FREQ=1700000       # 1.7 GHz, in kHz
RT_PRIORITY=98            # one below max FIFO priority; leaves headroom
                           # for the kernel's own critical RT threads (e.g.
                           # the watchdog) in case a benchmark misbehaves.

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

ARGS=(1.0 0.9 0.5)
RUNS_PER_ARG=3

CPUFREQ_DIR="/sys/devices/system/cpu/cpu${CPU_CORE}/cpufreq"

OLD_MIN_FREQ="$(cat "$CPUFREQ_DIR/scaling_min_freq")"
OLD_MAX_FREQ="$(cat "$CPUFREQ_DIR/scaling_max_freq")"
OLD_GOVERNOR="$(cat "$CPUFREQ_DIR/scaling_governor")"

# ============================================================
# Helper: safely set min/max frequency without ever writing a
# min > current max or a max < current min (which the kernel
# rejects). Picks whichever write order is safe given where the
# current values sit.
# ============================================================

set_freq_range() {
    local new_min="$1"
    local new_max="$2"
    local cur_max cur_min

    cur_max="$(cat "$CPUFREQ_DIR/scaling_max_freq")"
    if [[ "$new_min" -gt "$cur_max" ]]; then
        # Raising min above current max: max must go first.
        echo "$new_max" | sudo tee "$CPUFREQ_DIR/scaling_max_freq" >/dev/null
        echo "$new_min" | sudo tee "$CPUFREQ_DIR/scaling_min_freq" >/dev/null
    else
        cur_min="$(cat "$CPUFREQ_DIR/scaling_min_freq")"
        if [[ "$new_max" -lt "$cur_min" ]]; then
            # Lowering max below current min: min must go first.
            echo "$new_min" | sudo tee "$CPUFREQ_DIR/scaling_min_freq" >/dev/null
            echo "$new_max" | sudo tee "$CPUFREQ_DIR/scaling_max_freq" >/dev/null
        else
            # Either order is safe.
            echo "$new_min" | sudo tee "$CPUFREQ_DIR/scaling_min_freq" >/dev/null
            echo "$new_max" | sudo tee "$CPUFREQ_DIR/scaling_max_freq" >/dev/null
        fi
    fi
}

# ============================================================
# Cleanup / restore
# ============================================================

cleanup() {
    echo
    echo "=============================================="
    echo "Restoring CPU frequency configuration..."
    echo "=============================================="

    # Restore the original governor first, then the original
    # min/max range, using the safe order helper.
    echo "$OLD_GOVERNOR" | sudo tee "$CPUFREQ_DIR/scaling_governor" >/dev/null || true
    set_freq_range "$OLD_MIN_FREQ" "$OLD_MAX_FREQ" || true

    # Restore irqbalance.
    sudo systemctl start irqbalance >/dev/null 2>&1 || true

    echo "Original CPU frequency policy restored (governor: $OLD_GOVERNOR, " \
         "min: $OLD_MIN_FREQ kHz, max: $OLD_MAX_FREQ kHz)."
    echo "irqbalance restored."
}

trap cleanup EXIT INT TERM

# ============================================================
# Show configuration
# ============================================================

echo "=============================================="
echo "Benchmark configuration"
echo "=============================================="
echo "CPU core:       $CPU_CORE"
echo "Fixed frequency: $FIXED_FREQ kHz"
echo "RT priority:    $RT_PRIORITY"
echo "Runs/argument:  $RUNS_PER_ARG"
echo
echo "Original governor: $OLD_GOVERNOR"
echo "Original min:   $OLD_MIN_FREQ kHz"
echo "Original max:   $OLD_MAX_FREQ kHz"
echo

# ============================================================
# Stop irqbalance
#
# Note: this only halts *dynamic* IRQ rebalancing going forward.
# It does not move interrupts that are already affine to
# CPU_CORE. For full isolation, combine with isolcpus/nohz_full
# kernel boot params or explicit /proc/irq/*/smp_affinity writes.
# ============================================================

echo "=============================================="
echo "Stopping irqbalance..."
echo "=============================================="

sudo systemctl stop irqbalance >/dev/null 2>&1 || true

# ============================================================
# Build all binaries FIRST, with the shell's normal (unrestricted)
# CPU affinity, so the build can use all cores and isn't slowed
# down by being confined to CPU_CORE.
# ============================================================

echo "=============================================="
echo "Building all benchmark binaries..."
echo "=============================================="

RUSTFLAGS="-C target-cpu=native" cargo build --release --bins

# ============================================================
# Now pin this shell to the benchmark CPU, and fix its frequency.
# This happens AFTER the build so compilation isn't restricted
# to a single core.
# ============================================================

taskset -cp "$CPU_CORE" $$ >/dev/null

echo "=============================================="
echo "Setting CPU $CPU_CORE to fixed 1.7 GHz policy..."
echo "=============================================="

# Force an explicit governor that honors a pinned min==max range
# deterministically (avoids ambiguity between drivers/governors).
echo "userspace" | sudo tee "$CPUFREQ_DIR/scaling_governor" >/dev/null \
    || echo "performance" | sudo tee "$CPUFREQ_DIR/scaling_governor" >/dev/null

set_freq_range "$FIXED_FREQ" "$FIXED_FREQ"

echo
echo "CPU frequency policy after configuration:"
echo "governor: $(cat "$CPUFREQ_DIR/scaling_governor")"
echo "min: $(cat "$CPUFREQ_DIR/scaling_min_freq") kHz"
echo "max: $(cat "$CPUFREQ_DIR/scaling_max_freq") kHz"
echo

# ============================================================
# Benchmark
# ============================================================

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
            echo "CPU: $CPU_CORE"
            echo "Frequency policy: $(cat "$CPUFREQ_DIR/scaling_min_freq")-$(cat "$CPUFREQ_DIR/scaling_max_freq") kHz"
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
