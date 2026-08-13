#!/usr/bin/env python3
"""
analyze_probe_distances.py

Walks a fuzz_result/ directory containing fuzz_xx/ subfolders, each holding
~100 CSV run files (fuzz_xx_001.csv ... fuzz_xx_100.csv). Each CSV contains
7 snapshot blocks (25/50/75/90/95/99/100), each block a "snapshot N" header
line followed by "distance,count" rows.

For each fuzz_xx/ subfolder, produces 3 plots saved to fuzz_xx/plots/:

  1. <fuzz_xx>_distance_by_snapshot.png
     Pooled (across all runs) probe-distance distribution, log-y,
     one line per snapshot (7 lines total) -> shows whether the shape
     drifts across snapshots.

  2. <fuzz_xx>_pooled_distribution_stats.png
     Single pooled distribution (all runs + all snapshots merged),
     log-y bar chart, with vertical lines marking median / mean / p95 / p99.

  3. <fuzz_xx>_mean_by_run.png
     <fuzz_xx>_median_by_run.png
     <fuzz_xx>_p99_by_run.png
     Three separate files. Each shows run-to-run variation for one metric
     (mean / median / p99 probe distance): x-axis = snapshot, individual
     runs shown as jittered dots, plus a mean line across runs with a
     +/- 1 std dev shaded band.

Usage:
    python3 analyze_probe_distances.py /path/to/fuzz_result
"""

import argparse
import re
import sys
from pathlib import Path
from collections import defaultdict

import numpy as np
import pandas as pd
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

SNAPSHOT_RE = re.compile(r"^snapshot\s+(\d+)\s*$", re.IGNORECASE)
SNAPSHOT_ORDER = [25, 50, 75, 90, 95, 99, 100]


def parse_csv(path: Path) -> dict:
    """
    Parse a single run CSV into {snapshot_num: {distance: count}}.
    Tolerant of missing distance rows, blank lines, and stray whitespace.
    """
    blocks = {}
    current_snap = None
    with open(path, "r") as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line:
                continue
            m = SNAPSHOT_RE.match(line)
            if m:
                current_snap = int(m.group(1))
                blocks[current_snap] = {}
                continue
            if current_snap is None:
                continue  # stray data before any snapshot header; skip
            parts = line.split(",")
            if len(parts) != 2:
                continue
            try:
                dist = int(parts[0])
                count = int(parts[1])
            except ValueError:
                continue
            blocks[current_snap][dist] = blocks[current_snap].get(dist, 0) + count
    return blocks


def load_fuzz_dir(fuzz_dir: Path) -> pd.DataFrame:
    """
    Load all run CSVs in a fuzz_xx/ folder into a long-form DataFrame:
    columns = [run, snapshot, distance, count]
    """
    rows = []
    csv_files = sorted(fuzz_dir.glob(f"{fuzz_dir.name}_*.csv"))
    if not csv_files:
        # fall back to any csv in the folder, in case naming differs slightly
        csv_files = sorted(fuzz_dir.glob("*.csv"))

    for csv_path in csv_files:
        run_id = csv_path.stem
        blocks = parse_csv(csv_path)
        for snap, dist_counts in blocks.items():
            for dist, count in dist_counts.items():
                rows.append((run_id, snap, dist, count))

    if not rows:
        return pd.DataFrame(columns=["run", "snapshot", "distance", "count"])

    return pd.DataFrame(rows, columns=["run", "snapshot", "distance", "count"])


def weighted_percentile(distances: np.ndarray, counts: np.ndarray, pct: float) -> float:
    """
    Percentile of a (distance, count) histogram-style distribution.
    pct in [0, 100].
    """
    order = np.argsort(distances)
    d = distances[order]
    c = counts[order].astype(np.float64)
    cum = np.cumsum(c)
    total = cum[-1]
    if total == 0:
        return float("nan")
    target = pct / 100.0 * total
    idx = np.searchsorted(cum, target, side="left")
    idx = min(idx, len(d) - 1)
    return float(d[idx])


def weighted_mean(distances: np.ndarray, counts: np.ndarray) -> float:
    total = counts.sum()
    if total == 0:
        return float("nan")
    return float((distances * counts).sum() / total)


def _set_readable_xticks(ax, all_dists: np.ndarray, max_labels: int = 25):
    """
    Show every integer tick when the range is small, but thin out labels
    (while keeping all gridlines/ticks marks) when the distance range is
    large, so labels don't overlap into an unreadable blob.
    """
    n = len(all_dists)
    ax.set_xticks(all_dists)
    if n > max_labels:
        step = int(np.ceil(n / max_labels))
        labels = [str(d) if d % step == 0 else "" for d in all_dists]
        ax.set_xticklabels(labels)
        ax.tick_params(axis="x", labelrotation=0)
    else:
        ax.set_xticklabels([str(d) for d in all_dists])


# ---------------------------------------------------------------------------
# Plot 1: pooled distribution per snapshot, overlaid lines, log-y
# ---------------------------------------------------------------------------
def plot_distance_by_snapshot(df: pd.DataFrame, fuzz_name: str, out_path: Path):
    fig, ax = plt.subplots(figsize=(10, 6))

    pooled = df.groupby(["snapshot", "distance"], as_index=False)["count"].sum()
    max_dist = int(pooled["distance"].max())
    all_dists = np.arange(0, max_dist + 1)

    # High-contrast, visually unrelated colors (not a gradient/colormap)
    contrast_colors = ["red", "green", "deepskyblue", "gold", "black", "darkorange", "darkviolet"]
    snaps_present = [s for s in SNAPSHOT_ORDER if s in pooled["snapshot"].unique()]

    for i, snap in enumerate(snaps_present):
        sub = pooled[pooled["snapshot"] == snap].set_index("distance")["count"]
        y = sub.reindex(all_dists, fill_value=0).values.astype(np.float64)
        y_plot = np.where(y > 0, y, np.nan)  # avoid log(0) gaps
        color = contrast_colors[i % len(contrast_colors)]
        ax.plot(all_dists, y_plot, marker="o", markersize=4, linewidth=1.5,
                label=f"snapshot {snap}", color=color)

    ax.set_yscale("log")
    ax.set_xlabel("Probe distance")
    ax.set_ylabel("Slot count (log scale)")
    ax.set_title(f"{fuzz_name}: probe distance distribution by snapshot\n"
                 f"(pooled across all runs)")
    _set_readable_xticks(ax, all_dists)
    ax.grid(True, which="both", alpha=0.3)
    ax.legend(title="Snapshot", loc="upper right")
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# Plot 2: single pooled distribution (all runs + all snapshots) with stats
# ---------------------------------------------------------------------------
def plot_pooled_stats(df: pd.DataFrame, fuzz_name: str, out_path: Path) -> dict:
    pooled = df.groupby("distance", as_index=False)["count"].sum().sort_values("distance")
    distances = pooled["distance"].values
    counts = pooled["count"].values

    median = weighted_percentile(distances, counts, 50)
    p95 = weighted_percentile(distances, counts, 95)
    p99 = weighted_percentile(distances, counts, 99)
    mean = weighted_mean(distances, counts)

    fig, ax = plt.subplots(figsize=(10, 6))
    ax.bar(distances, counts, color="#4C72B0", width=0.8, zorder=2)
    ax.set_yscale("log")
    ax.set_xlabel("Probe distance")
    ax.set_ylabel("Slot count (log scale)")
    ax.set_title(f"{fuzz_name}: pooled probe distance distribution\n"
                 f"(all runs + all snapshots combined)")
    _set_readable_xticks(ax, distances)
    ax.grid(True, which="both", axis="y", alpha=0.3, zorder=0)

    stat_lines = [
        ("median", median, "black", "-"),
        ("mean", mean, "red", "--"),
        ("p95", p95, "orange", ":"),
        ("p99", p99, "purple", ":"),
    ]
    for label, value, color, style in stat_lines:
        ax.axvline(value, color=color, linestyle=style, linewidth=2,
                   label=f"{label} = {value:.3g}")

    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)

    return {"median": median, "mean": mean, "p95": p95, "p99": p99}


# ---------------------------------------------------------------------------
# Plot 3: box plot of per-run mean probe distance, grouped by snapshot
# ---------------------------------------------------------------------------
def plot_run_variation(per_run: pd.DataFrame, metric_col: str, metric_label: str,
                        fuzz_name: str, out_path: Path):
    """
    Readable alternative to a boxplot: jittered strip plot of every run's
    value at each snapshot (so individual runs are visible as dots),
    with a bold line connecting the per-snapshot mean, and a shaded
    band for +/- 1 std dev across runs.
    """
    snaps_present = [s for s in SNAPSHOT_ORDER if s in per_run["snapshot"].unique()]
    x_positions = np.arange(len(snaps_present))
    n_runs = per_run["run"].nunique()

    rng = np.random.default_rng(42)  # fixed seed -> reproducible jitter

    fig, ax = plt.subplots(figsize=(10, 6))

    means = []
    stds = []
    for x, snap in zip(x_positions, snaps_present):
        vals = per_run.loc[per_run["snapshot"] == snap, metric_col].dropna().values
        jitter = rng.uniform(-0.12, 0.12, size=len(vals))
        ax.scatter(np.full(len(vals), x) + jitter, vals,
                   color="#4C72B0", alpha=0.5, s=22, zorder=2,
                   label="individual runs" if x == 0 else None)
        means.append(np.mean(vals) if len(vals) else np.nan)
        stds.append(np.std(vals) if len(vals) else np.nan)

    means = np.array(means)
    stds = np.array(stds)

    ax.plot(x_positions, means, color="red", linewidth=2.5, marker="o",
            markersize=7, zorder=3, label="mean across runs")
    ax.fill_between(x_positions, means - stds, means + stds,
                    color="red", alpha=0.15, zorder=1, label="+/- 1 std dev")

    ax.set_xticks(x_positions)
    ax.set_xticklabels([str(s) for s in snaps_present])
    ax.set_xlabel("Snapshot")
    ax.set_ylabel(f"{metric_label} probe distance (per run)")
    ax.set_title(f"{fuzz_name}: run-to-run variation in {metric_label.lower()} probe distance\n"
                 f"(each dot = one of {n_runs} runs)")
    ax.grid(True, axis="y", alpha=0.3)
    ax.legend(loc="best")
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------
def process_fuzz_dir(fuzz_dir: Path):
    fuzz_name = fuzz_dir.name
    df = load_fuzz_dir(fuzz_dir)

    if df.empty:
        print(f"  [skip] {fuzz_name}: no parseable CSV data found")
        return

    n_runs = df["run"].nunique()
    n_snaps = df["snapshot"].nunique()
    print(f"  {fuzz_name}: {n_runs} runs, {n_snaps} snapshots, {len(df)} rows")

    plots_dir = fuzz_dir / "plots"
    plots_dir.mkdir(exist_ok=True)

    plot_distance_by_snapshot(
        df, fuzz_name, plots_dir / f"{fuzz_name}_distance_by_snapshot.png"
    )
    stats = plot_pooled_stats(
        df, fuzz_name, plots_dir / f"{fuzz_name}_pooled_distribution_stats.png"
    )

    # per (run, snapshot) stats, computed once, reused for all 3 run-variation plots
    def _per_group_stats(g):
        d = g["distance"].values
        c = g["count"].values
        return pd.Series({
            "mean_probe_distance": weighted_mean(d, c),
            "median_probe_distance": weighted_percentile(d, c, 50),
            "p99_probe_distance": weighted_percentile(d, c, 99),
        })

    per_run = (
        df.groupby(["run", "snapshot"])
        .apply(_per_group_stats, include_groups=False)
        .reset_index()
    )

    plot_run_variation(
        per_run, "mean_probe_distance", "Mean", fuzz_name,
        plots_dir / f"{fuzz_name}_mean_by_run.png"
    )
    plot_run_variation(
        per_run, "median_probe_distance", "Median", fuzz_name,
        plots_dir / f"{fuzz_name}_median_by_run.png"
    )
    plot_run_variation(
        per_run, "p99_probe_distance", "P99", fuzz_name,
        plots_dir / f"{fuzz_name}_p99_by_run.png"
    )

    print(f"    median={stats['median']:.3g} mean={stats['mean']:.3g} "
          f"p95={stats['p95']:.3g} p99={stats['p99']:.3g}")
    print(f"    -> plots saved to {plots_dir}/")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fuzz_result_dir", type=str,
                        help="Path to top-level fuzz_result/ directory")
    args = parser.parse_args()

    root = Path(args.fuzz_result_dir)
    if not root.is_dir():
        print(f"Error: {root} is not a directory", file=sys.stderr)
        sys.exit(1)

    fuzz_dirs = sorted([p for p in root.iterdir() if p.is_dir()])
    if not fuzz_dirs:
        print(f"No subdirectories found in {root}", file=sys.stderr)
        sys.exit(1)

    print(f"Found {len(fuzz_dirs)} fuzz_xx directories under {root}")
    for fuzz_dir in fuzz_dirs:
        process_fuzz_dir(fuzz_dir)

    print("Done.")


if __name__ == "__main__":
    main()
