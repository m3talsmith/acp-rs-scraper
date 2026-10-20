#!/usr/bin/env bash
# Lay out an installed tree under DESTDIR from a locally compiled binary.
# Shared by PKGBUILD, debian/rules and the RPM spec so every package has the
# same file layout. Usage: scripts/stage.sh DESTDIR [BINARY] [PREFIX]
set -euo pipefail
dest=${1:?usage: stage.sh DESTDIR [BINARY] [PREFIX]}
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${2:-$root/target/release/acp-red-star}
prefix=${3:-/usr}
name=acp-red-star
[ -x "$bin" ] || { echo "stage.sh: $bin not built (run cargo build --release --locked)" >&2; exit 1; }
d="$dest$prefix"
install -Dm755 "$bin" "$d/bin/$name"
alias="$root/target/release/acp-rs-scraper"
[ -x "$alias" ] || { echo "stage.sh: $alias not built (the compatibility name)" >&2; exit 1; }
install -Dm755 "$alias" "$d/bin/acp-rs-scraper"
short="$root/target/release/acprs"
[ -x "$short" ] || { echo "stage.sh: $short not built (the short command)" >&2; exit 1; }
install -Dm755 "$short" "$d/bin/acprs"
install -Dm644 "$root/LICENSE" "$d/share/licenses/$name/LICENSE"
for f in README.md CHANGELOG.md docs/USAGE.md docs/INSTALL.md assets/empty-body.NOTICE.md; do
  install -Dm644 "$root/$f" "$d/share/doc/$name/$(basename "$f")"
done
# The empty-Body animation converter (the built-in frames are inside the binary).
install -Dm755 "$root/tools/acp-anim" "$d/share/$name/tools/acp-anim"
install -d "$d/share/man/man1" "$d/share/bash-completion/completions" \
  "$d/share/zsh/site-functions" "$d/share/fish/vendor_completions.d"
"$bin" --generate man  > "$d/share/man/man1/$name.1"
"$bin" --generate bash > "$d/share/bash-completion/completions/$name"
"$bin" --generate zsh  > "$d/share/zsh/site-functions/_$name"
"$bin" --generate fish > "$d/share/fish/vendor_completions.d/$name.fish"
"$bin" --generate acprs-man  > "$d/share/man/man1/acprs.1"
"$bin" --generate acprs-bash > "$d/share/bash-completion/completions/acprs"
"$bin" --generate acprs-zsh  > "$d/share/zsh/site-functions/_acprs"
"$bin" --generate acprs-fish > "$d/share/fish/vendor_completions.d/acprs.fish"
chmod 644 "$d/share/man/man1/$name.1" "$d/share/bash-completion/completions/$name" \
  "$d/share/zsh/site-functions/_$name" "$d/share/fish/vendor_completions.d/$name.fish" \
  "$d/share/man/man1/acprs.1" "$d/share/bash-completion/completions/acprs" \
  "$d/share/zsh/site-functions/_acprs" "$d/share/fish/vendor_completions.d/acprs.fish"
