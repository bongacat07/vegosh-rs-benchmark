#!/usr/bin/env python3
"""
Generate Vegosh vs Hashbrown comparison charts from benchmark_results/.

Expected layout (from organize.sh):
    benchmark_results/{normal,jumbo}/lf_{48,75,87}/ratio_{1.0,0.9,0.5}/*.csv

Each CSV looks like:
    Overhead,34
    Max Keys,1000000
    run,min,p25,median,p75,p90,p95,p99,max,mean
    0,20,128,386,506,682,770,1310,45790,372.76
    ...
"""

import pandas as pd
import matplotlib.pyplot as plt
from pathlib import Path

# ---- Config ----------------------------------------------------------

BASE_DIR = Path("benchmark_results")
OUT_DIR = BASE_DIR / "plots"
OUT_DIR.mkdir(parents=True, exist_ok=True)

CATEGORIES = ["normal", "jumbo"]
LOAD_FACTORS = ["48", "75", "87"]
LF_LABELS = {"48": "47.68%", "75": "75%", "87": "87.5%"}
RATIOS = ["1.0", "0.9", "0.5"]   # row order, top to bottom
METRICS = ["mean", "median", "p99"]  # column order, left to right

VEG_COLOR = "#d62728"  # red
HB_COLOR = "#1f77b4"   # blue


# ---- Parsing -----------------------------------------------------------

def load_csv_stats(path: Path) -> dict:
    """Parse one benchmark CSV, subtract overhead, return avg mean/median/p99."""
    with open(path) as f:
        overhead_line = f.readline().strip()
        overhead = float(overhead_line.split(",")[1])
        f.readline()  # skip "Max Keys,..." line
        df = pd.read_csv(f)

    df["mean"] = df["mean"] - overhead
    df["median"] = df["median"] - overhead
    df["p99"] = df["p99"] - overhead

    return {
        "mean": df["mean"].mean(),
        "median": df["median"].mean(),
        "p99": df["p99"].mean(),
    }


def find_file(folder: Path, keyword: str) -> Path | None:
    matches = [p for p in folder.glob("*.csv") if keyword in p.stem.lower()]
    if not matches:
        return None
    if len(matches) > 1:
        print(f"WARNING: multiple '{keyword}' files in {folder}, using {matches[0].name}")
    return matches[0]


# ---- Plotting ------------------------------------------------------------

def build_figure(category: str, lf: str):
    fig, axes = plt.subplots(
        nrows=len(RATIOS), ncols=len(METRICS),
        figsize=(13, 11),
    )

    fig.suptitle(
        f"Vegosh vs Hashbrown ({category.capitalize()})\nLoad Factor {LF_LABELS[lf]}",
        fontsize=16, fontweight="bold", y=0.98,
    )

    any_data_found = False

    for row_idx, ratio in enumerate(RATIOS):
        folder = BASE_DIR / category / f"lf_{lf}" / f"ratio_{ratio}"

        veg_stats = hb_stats = None
        if folder.exists():
            veg_file = find_file(folder, "veg")
            hb_file = find_file(folder, "hb")
            if veg_file:
                veg_stats = load_csv_stats(veg_file)
            if hb_file:
                hb_stats = load_csv_stats(hb_file)

        for col_idx, metric in enumerate(METRICS):
            ax = axes[row_idx][col_idx]

            if veg_stats is None or hb_stats is None:
                ax.text(0.5, 0.5, "No data", ha="center", va="center",
                        transform=ax.transAxes, color="gray")
                ax.set_xticks([])
                ax.set_yticks([])
                continue

            any_data_found = True
            veg_val = veg_stats[metric]
            hb_val = hb_stats[metric]

            bars = ax.bar(
                ["Vegosh", "Hashbrown"],
                [veg_val, hb_val],
                color=[VEG_COLOR, HB_COLOR],
                width=0.6,
            )

            # Scale y-axis per panel so small values aren't flattened by outliers
            max_val = max(veg_val, hb_val)
            ax.set_ylim(0, max_val * 1.25)

            for bar, val in zip(bars, [veg_val, hb_val]):
                ax.text(
                    bar.get_x() + bar.get_width() / 2, val + max_val * 0.02,
                    f"{val:.1f}", ha="center", va="bottom", fontsize=9,
                )

            if col_idx == 0:
                ax.set_ylabel("CPU cycles", fontsize=10)

            if row_idx == 0:
                ax.set_title(metric.capitalize(), fontsize=12, fontweight="bold")

        axes[row_idx][0].annotate(
            f"Hit/miss Ratio ({ratio})",
            xy=(-0.35, 0.5), xycoords="axes fraction",
            fontsize=11, fontweight="bold",
            ha="right", va="center", rotation=90,
        )

    if not any_data_found:
        print(f"SKIPPED (no data at all): {category}/lf_{lf}")
        plt.close(fig)
        return

    fig.tight_layout(rect=(0.03, 0, 1, 0.94))
    out_path = OUT_DIR / f"{category}_lf{lf}.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"Saved: {out_path}")


# ---- Main ----------------------------------------------------------------

def main():
    for category in CATEGORIES:
        for lf in LOAD_FACTORS:
            build_figure(category, lf)


if __name__ == "__main__":
    main()
