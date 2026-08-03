#!/usr/bin/env python3
"""
Build stacked landing/snapshot histogram plots for every CSV in a directory.

Expected CSV structure (one file per histogram set):
landings
1,1032798
2,513864
...
snapshot
1,367708
2,329540
...
"""
import csv
import sys
from pathlib import Path

import matplotlib.pyplot as plt


def parse_sectioned_csv(path: Path) -> dict[str, list[tuple[int, int]]]:
    """Parse a csv with two labeled sections: 'landings' and 'snapshot'."""
    sections: dict[str, list[tuple[int, int]]] = {}
    current: str | None = None

    with path.open(newline="") as f:
        reader = csv.reader(f)
        for row in reader:
            if not row:
                continue
            if len(row) == 1:
                # section header line, e.g. "landings" or "snapshot"
                current = row[0].strip().lower()
                sections[current] = []
            else:
                if current is None:
                    raise ValueError(
                        f"{path}: data row {row!r} found before any section header"
                    )
                bin_, count = row[0], row[1]
                sections[current].append((int(bin_), int(count)))
    return sections


def weighted_stats(data: list[tuple[int, int]]) -> dict[str, float]:
    """Compute mean, median, p95, p99 from (bin, count) pairs without expanding."""
    data = sorted(data, key=lambda pair: pair[0])
    total = sum(count for _, count in data)

    # mean
    mean = sum(bin_ * count for bin_, count in data) / total

    def percentile(p: float) -> float:
        target = p * total
        cum = 0
        for bin_, count in data:
            cum += count
            if cum >= target:
                return float(bin_)
        return float(data[-1][0])

    return {
        "mean": mean,
        "median": percentile(0.50),
        "p95": percentile(0.95),
        "p99": percentile(0.99),
    }


def annotate_stats(ax, stats: dict[str, float]):
    text = (
        f"mean={stats['mean']:.2f}  "
        f"median={stats['median']:.0f}  "
        f"p95={stats['p95']:.0f}  "
        f"p99={stats['p99']:.0f}"
    )
    ax.text(
        0.98, 0.95, text,
        transform=ax.transAxes,
        ha="right", va="top",
        fontsize=9,
        bbox=dict(boxstyle="round", facecolor="white", alpha=0.8, edgecolor="gray"),
    )


def plot_histograms(path: Path, sections: dict, out_dir: Path):
    fig, (ax_land, ax_snap) = plt.subplots(2, 1, figsize=(8, 8))

    out_name = path.stem.replace("hist", "Vegosh")

    if "landings" in sections:
        x, y = zip(*sections["landings"])
        ax_land.bar(x, y, color="red", width=0.8)
        ax_land.set_yscale("log")
        ax_land.set_title(f"{out_name} — landings")
        ax_land.set_xlabel("bin")
        ax_land.set_ylabel("count (log scale)")
        annotate_stats(ax_land, weighted_stats(sections["landings"]))

    if "snapshot" in sections:
        x, y = zip(*sections["snapshot"])
        ax_snap.bar(x, y, color="blue", width=0.8)
        ax_snap.set_yscale("log")
        ax_snap.set_title(f"{out_name} — snapshot")
        ax_snap.set_xlabel("bin")
        ax_snap.set_ylabel("count (log scale)")
        annotate_stats(ax_snap, weighted_stats(sections["snapshot"]))

    fig.tight_layout()
    out_path = out_dir / f"{out_name}.png"
    fig.savefig(out_path, dpi=150)
    plt.close(fig)
    print(f"wrote {out_path}")


def main():
    hist_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    out_dir = hist_dir / "plots"
    out_dir.mkdir(exist_ok=True)

    csv_files = sorted(hist_dir.glob("*.csv"))
    if not csv_files:
        print(f"no csv files found in {hist_dir}")
        return

    for csv_path in csv_files:
        sections = parse_sectioned_csv(csv_path)
        plot_histograms(csv_path, sections, out_dir)


if __name__ == "__main__":
    main()
