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

# React 18.2.0 — unminified development build (~300 KB)
download \
    "https://unpkg.com/react@18.2.0/umd/react.development.js" \
    "react.development.js"

# jQuery 3.7.1 — unminified (~290 KB)
download \
    "https://code.jquery.com/jquery-3.7.1.js" \
    "jquery-3.7.1.js"

# Three.js r160 minified — large bundle (~660 KB)
download \
    "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js" \
    "three.min.js"

echo "Done. Fixture sizes:"
wc -c "$FIXTURES_DIR"/*.js
