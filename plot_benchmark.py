#!/usr/bin/env python3
"""
Generate Vegosh vs Hashbrown comparison charts from benchmark_results/.

Expected layout (from organize.sh):
    benchmark_results/{normal,jumbo}/lf_{48,75,87}/ratio_{1.0,0.9,0.5}/*.csv

Each CSV now looks like:
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

# ---- Dark theme palette ------------------------------------------------

BG_COLOR = "#0d1117"        # page background (GitHub-dark style)
PANEL_COLOR = "#161b22"     # per-axes background
GRID_COLOR = "#30363d"
TEXT_COLOR = "#e6edf3"
SUBTEXT_COLOR = "#8b949e"

VEG_COLOR = "#ff5c5c"   # red (Vegosh)
VEG_EDGE = "#ff8080"
HB_COLOR = "#4da3ff"    # blue (Hashbrown)
HB_EDGE = "#7cbcff"

plt.rcParams.update({
    "figure.facecolor": BG_COLOR,
    "axes.facecolor": PANEL_COLOR,
    "axes.edgecolor": GRID_COLOR,
    "axes.labelcolor": TEXT_COLOR,
    "text.color": TEXT_COLOR,
    "xtick.color": SUBTEXT_COLOR,
    "ytick.color": SUBTEXT_COLOR,
    "grid.color": GRID_COLOR,
    "font.family": "DejaVu Sans",
})


# ---- Parsing -----------------------------------------------------------
def load_csv_stats(path: Path) -> dict:
    """Parse one benchmark CSV, return avg mean/median/p99."""
    with open(path) as f:
        f.readline()  # skip "Overhead,..." line
        f.readline()  # skip "Max Keys,..." line
        df = pd.read_csv(f)

    if "mean" not in df.columns:
        raise ValueError(
            f"'mean' column not found in {path}. "
            f"Columns seen: {list(df.columns)}. "
            f"File may still have header/metadata lines before the CSV header row."
        )

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
        figsize=(14, 11.5),
    )
    fig.patch.set_facecolor(BG_COLOR)

    fig.suptitle(
        f"Vegosh vs Hashbrown  ·  {category.capitalize()}\nLoad Factor {LF_LABELS[lf]}",
        fontsize=17, fontweight="bold", y=0.985, color=TEXT_COLOR,
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
            ax.set_facecolor(PANEL_COLOR)

            if veg_stats is None or hb_stats is None:
                ax.text(0.5, 0.5, "No data", ha="center", va="center",
                        transform=ax.transAxes, color=SUBTEXT_COLOR, fontsize=10)
                ax.set_xticks([])
                ax.set_yticks([])
                for spine in ax.spines.values():
                    spine.set_color(GRID_COLOR)
                continue

            any_data_found = True
            veg_val = veg_stats[metric]
            hb_val = hb_stats[metric]

            bars = ax.bar(
                ["Vegosh", "Hashbrown"],
                [veg_val, hb_val],
                color=[VEG_COLOR, HB_COLOR],
                edgecolor=[VEG_EDGE, HB_EDGE],
                linewidth=1.4,
                width=0.55,
                zorder=3,
            )

            max_val = max(veg_val, hb_val)
            ax.set_ylim(0, max_val * 1.28)

            ax.grid(axis="y", linewidth=0.6, alpha=0.5, zorder=0)
            ax.set_axisbelow(True)

            for spine_name, spine in ax.spines.items():
                if spine_name in ("top", "right"):
                    spine.set_visible(False)
                else:
                    spine.set_color(GRID_COLOR)

            for bar, val in zip(bars, [veg_val, hb_val]):
                ax.text(
                    bar.get_x() + bar.get_width() / 2, val + max_val * 0.03,
                    f"{val:.1f}", ha="center", va="bottom",
                    fontsize=9.5, color=TEXT_COLOR, fontweight="medium",
                )

            ax.tick_params(axis="x", labelsize=9.5, colors=SUBTEXT_COLOR)
            ax.tick_params(axis="y", labelsize=8.5, colors=SUBTEXT_COLOR)

            if col_idx == 0:
                ax.set_ylabel("CPU cycles", fontsize=10, color=SUBTEXT_COLOR)

            if row_idx == 0:
                ax.set_title(metric.capitalize(), fontsize=13, fontweight="bold",
                             color=TEXT_COLOR, pad=10)

        axes[row_idx][0].annotate(
            f"Ratio {ratio}",
            xy=(-0.42, 0.5), xycoords="axes fraction",
            fontsize=12, fontweight="bold", color=SUBTEXT_COLOR,
            ha="right", va="center", rotation=90,
        )

    # Legend
    from matplotlib.patches import Patch
    legend_handles = [
        Patch(facecolor=VEG_COLOR, edgecolor=VEG_EDGE, label="Vegosh"),
        Patch(facecolor=HB_COLOR, edgecolor=HB_EDGE, label="Hashbrown"),
    ]
    fig.legend(
        handles=legend_handles, loc="upper right",
        bbox_to_anchor=(0.98, 0.985), fontsize=11,
        frameon=False, labelcolor=TEXT_COLOR,
    )

    if not any_data_found:
        print(f"SKIPPED (no data at all): {category}/lf_{lf}")
        plt.close(fig)
        return

    fig.tight_layout(rect=(0.03, 0, 1, 0.93))
    out_path = OUT_DIR / f"{category}_lf{lf}.png"
    fig.savefig(out_path, dpi=160, facecolor=BG_COLOR)
    plt.close(fig)
    print(f"Saved: {out_path}")


# ---- Main ----------------------------------------------------------------

def main():
    for category in CATEGORIES:
        for lf in LOAD_FACTORS:
            build_figure(category, lf)


if __name__ == "__main__":
    main()
