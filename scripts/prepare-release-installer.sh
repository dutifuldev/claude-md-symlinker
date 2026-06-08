#!/bin/sh
set -eu

version="${1:-latest}"
repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
template="$repo_root/scripts/claude-md-symlinker-installer.sh"
output="$repo_root/target/distrib/claude-md-symlinker-installer.sh"

mkdir -p "$(dirname -- "$output")"
sed "s/__CLAUDE_MD_SYMLINKER_VERSION__/$version/g" "$template" > "$output"
chmod +x "$output"
