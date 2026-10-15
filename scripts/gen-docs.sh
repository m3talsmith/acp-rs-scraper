#!/usr/bin/env bash
# Regenerate rustdoc into docs/api/ and a docs/index.html redirect.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo doc --no-deps --document-private-items
rm -rf docs/api
mkdir -p docs
cp -r target/doc docs/api
crate=acp_rs_scraper
cat > docs/index.html <<HTML
<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<title>acp-rs-scraper docs</title>
<meta http-equiv="refresh" content="0; url=api/${crate}/index.html">
<link rel="canonical" href="api/${crate}/index.html">
</head><body>
<p><a href="api/${crate}/index.html">API docs</a> · <a href="USAGE.md">Usage guide</a></p>
</body></html>
HTML
echo "docs: docs/index.html -> docs/api/${crate}/index.html"
