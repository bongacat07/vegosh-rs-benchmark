#!/usr/bin/env bash
set -euo pipefail

echo "=============================================="
echo "Step 0: Setting CPU to performance mode"
echo "=============================================="
sudo cpupower frequency-set -g performance
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

echo "=============================================="
echo "Step 1: Build & run all benchmarks"
echo "=============================================="
./bench.sh

echo "=============================================="
echo "Step 2: Organize CSV results"
echo "=============================================="
./organize.sh

echo "=============================================="
echo "Step 3: Generate plots"
echo "=============================================="
python3 plot_benchmark.py

echo "=============================================="
echo "All done. Plots are in benchmark_results/plots/"
echo "=============================================="
