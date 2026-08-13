#!/usr/bin/env bash
#
# fuzz_parallel.sh — parallel seed-sweep fuzz runner
#
# Builds insert_fuzz_{48,75,87} in release mode, then fans out 4 worker
# processes per binary (12 workers total). Each worker sweeps a fixed
# 25-seed chunk over the range 1..100, invoking the binary once per seed
# (seed passed as a single u64 arg). Each worker writes its own log file
# and tracks per-seed non-zero exit codes as failures.
#
# No CPU pinning (taskset) — left to the OS scheduler.
#
# Usage: ./fuzz_parallel.sh

set -uo pipefail

BIN_DIR="target/release"
BINARIES=(insert_fuzz_48 insert_fuzz_75 insert_fuzz_87)
CHUNKS=(1:25 26:50 51:75 76:100)

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_DIR="fuzz_logs_${TIMESTAMP}"
mkdir -p "$LOG_DIR"

echo "==> cargo build --release"
if ! cargo build --release --bin insert_fuzz_48 --bin insert_fuzz_75 --bin insert_fuzz_87; then
    echo "!! build failed, aborting" >&2
    exit 1
fi

for b in "${BINARIES[@]}"; do
    if [[ ! -x "${BIN_DIR}/${b}" ]]; then
        echo "!! missing binary after build: ${BIN_DIR}/${b}" >&2
        exit 1
    fi
done

# Runs a single worker: sweeps [start,end] seeds against one binary,
# logging every seed result. Exits with the count of failed seeds
# (0 == all passed), which becomes the backgrounded job's exit status.
run_chunk() {
    local bin="$1" start="$2" end="$3" logfile="$4"
    local fail_count=0
    {
        echo "[$(date '+%H:%M:%S')] start pid=$$ bin=${bin} seeds=${start}-${end}"
        for ((seed = start; seed <= end; seed++)); do
            if "${BIN_DIR}/${bin}" "$seed"; then
                echo "[$(date '+%H:%M:%S')] seed=${seed} OK"
            else
                rc=$?
                echo "[$(date '+%H:%M:%S')] seed=${seed} FAIL exit=${rc}"
                fail_count=$((fail_count + 1))
            fi
        done
        echo "[$(date '+%H:%M:%S')] end pid=$$ bin=${bin} seeds=${start}-${end} failures=${fail_count}"
        exit "$fail_count"
    } &> "$logfile"
}

pids=()
labels=()

echo "==> launching 12 workers (3 binaries x 4 chunks of 25 seeds)"
for bin in "${BINARIES[@]}"; do
    idx=0
    for chunk in "${CHUNKS[@]}"; do
        idx=$((idx + 1))
        start="${chunk%%:*}"
        end="${chunk##*:}"
        logfile="${LOG_DIR}/${bin}_p${idx}_seeds${start}-${end}.log"
        run_chunk "$bin" "$start" "$end" "$logfile" &
        pids+=("$!")
        labels+=("${bin} p${idx} [${start}-${end}] -> ${logfile}")
    done
done

echo "==> waiting on ${#pids[@]} workers..."
echo

fail_total=0
for i in "${!pids[@]}"; do
    pid="${pids[$i]}"
    label="${labels[$i]}"
    if wait "$pid"; then
        echo "  OK   ${label}"
    else
        rc=$?
        echo "  FAIL ${label} -> ${rc} bad seed(s)"
        fail_total=$((fail_total + rc))
    fi
done

echo
echo "==> summary"
echo "logs: ${LOG_DIR}/"
if [[ $fail_total -eq 0 ]]; then
    echo "all 300 seed runs passed (3 binaries x 100 seeds)"
    exit 0
else
    echo "${fail_total} seed run(s) failed across all workers — check logs above"
    exit 1
fi
