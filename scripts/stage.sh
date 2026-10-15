#!/usr/bin/env bash
# Lay out an installed tree under DESTDIR from a locally compiled binary.
# Shared by PKGBUILD, debian/rules and the RPM spec so every package has the
# same file layout. Usage: scripts/stage.sh DESTDIR [BINARY] [PREFIX]
set -euo pipefail
dest=${1:?usage: stage.sh DESTDIR [BINARY] [PREFIX]}
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${2:-$root/target/release/acp-rs-scraper}
prefix=${3:-/usr}
name=acp-rs-scraper
[ -x "$bin" ] || { echo "stage.sh: $bin not built (run cargo build --release --locked)" >&2; exit 1; }
d="$dest$prefix"
install -Dm755 "$bin" "$d/bin/$name"
install -Dm644 "$root/LICENSE" "$d/share/licenses/$name/LICENSE"
for f in README.md CHANGELOG.md docs/USAGE.md docs/INSTALL.md; do
  install -Dm644 "$root/$f" "$d/share/doc/$name/$(basename "$f")"
done
install -d "$d/share/man/man1" "$d/share/bash-completion/completions" \
  "$d/share/zsh/site-functions" "$d/share/fish/vendor_completions.d"
"$bin" --generate man  > "$d/share/man/man1/$name.1"
"$bin" --generate bash > "$d/share/bash-completion/completions/$name"
"$bin" --generate zsh  > "$d/share/zsh/site-functions/_$name"
"$bin" --generate fish > "$d/share/fish/vendor_completions.d/$name.fish"
chmod 644 "$d/share/man/man1/$name.1" "$d/share/bash-completion/completions/$name" \
  "$d/share/zsh/site-functions/_$name" "$d/share/fish/vendor_completions.d/$name.fish"
