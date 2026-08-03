#!/usr/bin/env python3

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

    if "snapshot" in sections:
        x, y = zip(*sections["snapshot"])
        ax_snap.bar(x, y, color="blue", width=0.8)
        ax_snap.set_yscale("log")
        ax_snap.set_title(f"{out_name} — snapshot")
        ax_snap.set_xlabel("bin")
        ax_snap.set_ylabel("count (log scale)")

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
