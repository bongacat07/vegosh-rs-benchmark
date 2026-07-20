#!/usr/bin/env bash
set -euo pipefail

BASE_DIR="benchmark_results"

for file in benchmark_results_*.csv; do
    [[ -e "$file" ]] || continue  # skip if no matches

    # Strip prefix/suffix to get the middle part
    name="${file#benchmark_results_}"
    name="${name%.csv}"

    # Detect jumbo vs normal
    if [[ "$name" == *"_jumbo_"* ]]; then
        category="jumbo"
        name="${name/_jumbo_/_}"   # remove "jumbo" marker for easier parsing
    else
        category="normal"
    fi

    # name is now like: hb_48_0.5  or veg_87_1
    # Extract size (48/75/87) and ratio (last field)
    if [[ "$name" =~ ^([a-z]+)_([0-9]+)_([0-9.]+)$ ]]; then
        size="${BASH_REMATCH[2]}"
        ratio="${BASH_REMATCH[3]}"
    else
        echo "WARNING: couldn't parse '$file', skipping"
        continue
    fi

    # Normalize ratio: "1" -> "1.0"
    if [[ "$ratio" == "1" ]]; then
        ratio="1.0"
    fi

    dest_dir="$BASE_DIR/$category/lf_${size}/ratio_${ratio}"
    mkdir -p "$dest_dir"
    mv "$file" "$dest_dir/"
    echo "Moved $file -> $dest_dir/"
done

echo "Done organizing into $BASE_DIR/"
