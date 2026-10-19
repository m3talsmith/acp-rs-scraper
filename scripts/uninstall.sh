#!/usr/bin/env bash
# Uninstall acp-rs-scraper AND (by default) purge its per-user data in ~/.acp.
#   scripts/uninstall.sh              remove package + purge data (asks first)
#   scripts/uninstall.sh --no-purge   remove package, keep ~/.acp intact
#   -y/--yes skips the prompt (required when stdin is not a terminal)
#   --purge is accepted as a no-op alias (purging is the default)
# pacman -R / apt remove / dnf remove alone never touch home directories;
# this script is the way to purge user data.
set -euo pipefail
name=acp-rs-scraper
purge=1 yes=0
for a in "$@"; do
  case $a in
    --no-purge) purge=0 ;;
    --purge) ;;
    -y|--yes) yes=1 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done
say() { echo "uninstall: $*" >&2; }
SUDO=; [ "$(id -u)" = 0 ] || SUDO=sudo
# Purge the invoking user's data even under sudo.
home=$HOME
if [ "$(id -u)" = 0 ] && [ -n "${SUDO_USER:-}" ]; then home=$(getent passwd "$SUDO_USER" | cut -d: -f6); fi
acp="$home/.acp"
data=("$acp/ai/data/rs.database" "$acp/ai/data/rs.database.bootstrap" "$acp/ai/data/rs.database-wal" "$acp/ai/data/rs.database-shm"
      "$acp/ai/data/rs.database-journal" "$acp/ai/data/attachments" "$acp/ai/data/exports" "$acp/preferences.toml"
      "$acp/recovery.seal" "$acp/recovery-next")

# ---- how is it installed?
how=() manifests=()
pacman -Qq $name >/dev/null 2>&1 && how+=(pacman)
dpkg-query -W -f='${Status}' $name 2>/dev/null | grep -q 'install ok installed' && how+=(apt)
rpm -q $name >/dev/null 2>&1 && how+=(dnf)
for m in "$home/.local/share/$name/install-manifest" /usr/local/share/$name/install-manifest; do
  [ -f "$m" ] && manifests+=("$m")
done

present=(); for p in "${data[@]}"; do [ -e "$p" ] && present+=("$p"); done
[ ${#how[@]} = 0 ] && [ ${#manifests[@]} = 0 ] && say "no installed package or manifest found"
[ $purge = 1 ] && [ ${#present[@]} = 0 ] && say "no user data under $acp"

# ---- one confirmation listing everything
echo "This will remove:" >&2
for h in ${how[@]+"${how[@]}"}; do echo "  package $name (via $h)" >&2; done
for m in ${manifests[@]+"${manifests[@]}"}; do echo "  files listed in $m" >&2; done
if [ $purge = 1 ]; then
  for p in ${present[@]+"${present[@]}"}; do echo "  $p ($(du -sh "$p" 2>/dev/null | cut -f1))" >&2; done
  [ ${#present[@]} -gt 0 ] && echo "  (database, attachments, exports and preferences.toml — use --no-purge to keep them)" >&2
else
  echo "  (keeping $acp — --no-purge)" >&2
fi
if [ ${#how[@]} = 0 ] && [ ${#manifests[@]} = 0 ] && { [ $purge = 0 ] || [ ${#present[@]} = 0 ]; }; then
  say "nothing to do"; exit 0
fi
if [ $yes = 0 ]; then
  [ -t 0 ] || { say "refusing without a terminal (pass --yes)"; exit 1; }
  read -r -p "Type 'yes' to continue: " r
  [ "$r" = yes ] || { say "aborted; nothing removed"; exit 1; }
fi

for h in ${how[@]+"${how[@]}"}; do
  case $h in
    pacman) $SUDO pacman -R --noconfirm $name ;;
    apt) $SUDO apt-get remove -y $name ;;
    dnf) $SUDO dnf remove -y $name ;;
  esac
done
for m in ${manifests[@]+"${manifests[@]}"}; do
  S=; case $m in /usr/*) S=$SUDO ;; esac
  while read -r f; do [ -n "$f" ] && $S rm -f -- "$f"; done < "$m"
  $S rmdir --ignore-fail-on-non-empty "$(dirname "$m")" 2>/dev/null || true
done
if [ $purge = 1 ]; then
  for p in ${present[@]+"${present[@]}"}; do rm -rf -- "$p"; done
  rmdir "$acp/ai/data" "$acp/ai" "$acp" 2>/dev/null || true   # only if now empty
fi
say "done"
