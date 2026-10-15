#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
audit_dir=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/mobile-audit.XXXXXX")
trap 'rm -rf "$audit_dir"' EXIT
export OSV_SCALIBR_LOCAL_DB_CACHE_DIRECTORY="$audit_dir/db"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform=darwin_arm64; checksum=d4926ef8da134332543be276dff157e4c170f99ad91fc48e89448a8816bdf70b ;;
  Darwin-x86_64) platform=darwin_amd64; checksum=e82c5df7ab0a42dff29a1cd0660711447b56b4c0483ff66ad8634df10fd81a45 ;;
  Linux-x86_64) platform=linux_amd64; checksum=160c6253e2922e9e0a6a9771c5c4c40a4b3c6090b34a3cdd5e80c96cc2e4218b ;;
  *) echo 'Unsupported OSV scanner platform' >&2; exit 1 ;;
esac
python3 scripts/browser-sbom.py > "$audit_dir/browser.cdx.json"
mkdir -p "$audit_dir/db/osv-scalibr/crates.io"
curl -fLsS --retry 3 https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip -o "$audit_dir/db/osv-scalibr/crates.io/all.zip"
curl -fLsS --retry 3 "https://github.com/google/osv-scanner/releases/download/v2.7.0/osv-scanner_$platform" -o "$audit_dir/osv-scanner"
echo "$checksum  $audit_dir/osv-scanner" | shasum -a 256 --check -
chmod +x "$audit_dir/osv-scanner"
"$audit_dir/osv-scanner" scan source --offline -L "$audit_dir/browser.cdx.json" --config .github/osv-scanner.toml --format gh-annotations
