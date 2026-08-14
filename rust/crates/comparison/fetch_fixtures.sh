#!/usr/bin/env bash
# fetch_fixtures.sh — downloads plain-JS benchmark fixtures.
# Run once before benchmarking:
#   bash rust/crates/comparison/fetch_fixtures.sh
#
# Files are saved into rust/crates/comparison/fixtures/ and are gitignored.
# Only plain JavaScript files — NO TypeScript/JSX.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"

mkdir -p "$FIXTURES_DIR"

download() {
    local url="$1"
    local dest="$FIXTURES_DIR/$2"
    if [ -f "$dest" ]; then
        echo "  already present: $2"
    else
        echo "  downloading: $2"
        curl -fsSL "$url" -o "$dest"
    fi
}

echo "Fetching fixtures into $FIXTURES_DIR ..."

# React 18.2.0 — unminified development build (~110 KB)
download \
    "https://unpkg.com/react@18.2.0/umd/react.development.js" \
    "react.development.js"

# jQuery 3.7.1 — unminified (~285 KB)
download \
    "https://code.jquery.com/jquery-3.7.1.js" \
    "jquery-3.7.1.js"

# Three.js r160 minified — large bundle (~670 KB)
download \
    "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js" \
    "three.min.js"

# TypeScript 5.4.5 — compiled TypeScript compiler in plain JS (~8–9 MB)
download \
    "https://cdn.jsdelivr.net/npm/typescript@5.4.5/lib/typescript.js" \
    "typescript.js"

echo ""
echo "Generating trailing-error variants..."

# Generate .err.js variant for each fixture: append guaranteed syntax error
for js_file in "$FIXTURES_DIR"/*.js; do
    err_file="${js_file%.js}.err.js"
    cp "$js_file" "$err_file"
    printf '\nvar __bench_parse_error__ = ;\n' >> "$err_file"
    echo "  created: $(basename "$err_file")"
done

echo ""
echo "Done. Fixture sizes:"
ls -lh "$FIXTURES_DIR"/*.js | awk '{print $5 "\t" $9}'
