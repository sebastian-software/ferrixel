#!/usr/bin/env bash
# CI-only, deterministic performance feasibility smoke on a synthetic SDR image.
# This is NOT a color- or perceptual-quality-matched Sharp/libvips benchmark.
set -euo pipefail
export LC_ALL=C

command -v vipsthumbnail >/dev/null || { echo "vipsthumbnail required" >&2; exit 2; }
command -v vips >/dev/null || { echo "libvips required" >&2; exit 2; }
command -v cargo >/dev/null || { echo "Rust cargo required" >&2; exit 2; }
[[ -x /usr/bin/time ]] || { echo "GNU time required" >&2; exit 2; }

repo_dir=$(pwd)
mkdir -p "$repo_dir/target"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
python3 - "$tmp/source.ppm" <<'PY'
from pathlib import Path
import sys

width, height = 1280, 853
with Path(sys.argv[1]).open("wb") as file:
    file.write(f"P6\n{width} {height}\n255\n".encode("ascii"))
    for y in range(height):
        line = bytearray(width * 3)
        for x in range(width):
            i = x * 3
            line[i] = (x * 11 + y * 3) & 255
            line[i + 1] = ((x // 3) ^ (y * 5)) & 255
            line[i + 2] = ((x * x // 71) + y) & 255
        file.write(line)
PY
vips copy "$tmp/source.ppm" "$tmp/source.jpg[Q=90]"

{
    echo "# M1 synthetic performance smoke; NOT equivalent image fidelity"
    echo "# Synthetic input: 1280x853 deterministic gradient; JPEG Q=90"
    echo "# Output 640x427, 2 iterations each; encoder quality labels not comparable"
    uname -a
    rustc --version
    vips --version
    echo "# CPU: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2-)"
} > "$repo_dir/target/m1-benchmark-environment.txt"

cargo build --release --features cli
csv="$repo_dir/target/m1-benchmark-smoke.csv"
echo "backend,iteration,elapsed_seconds,peak_rss_kib,output_bytes,comparable_quality" > "$csv"
for iteration in 1 2; do
    /usr/bin/time -f '%e,%M' -o "$tmp/ferrixel-time" \
        "$repo_dir/target/release/ferrixel" \
        "$tmp/source.jpg" "$tmp/ferrixel.avif" 640 427 \
        --assume-srgb --quality=80 --speed=10 --budget-mb=192 \
        > "$tmp/ferrixel-log"
    IFS=, read -r elapsed rss < "$tmp/ferrixel-time"
    bytes=$(wc -c < "$tmp/ferrixel.avif" | tr -d ' ')
    printf 'ferrixel,%s,%s,%s,%s,no\n' "$iteration" "$elapsed" "$rss" "$bytes" >> "$csv"

    # libvips uses a different filter, color pipeline and AV1 encoder.
    # The result is useful as a feasibility comparator only.
    if /usr/bin/time -f '%e,%M' -o "$tmp/vips-time" \
        vipsthumbnail "$tmp/source.jpg" --linear --size=640x427 \
        --output="$tmp/vips.avif[Q=80,effort=0]" \
        > "$tmp/vips-log" 2>&1; then
        IFS=, read -r elapsed rss < "$tmp/vips-time"
        bytes=$(wc -c < "$tmp/vips.avif" | tr -d ' ')
        printf 'libvips,%s,%s,%s,%s,no\n' "$iteration" "$elapsed" "$rss" "$bytes" >> "$csv"
    else
        printf 'libvips,%s,,,,no\n' "$iteration" >> "$csv"
        echo "libvips AVIF output was unavailable in this runner:" >&2
        cat "$tmp/vips-log" >&2
    fi
done

echo "M1 smoke results (not fair equal-quality performance claims):"
cat "$csv"
