#!/usr/bin/env bash
# Build acp-red-star FROM SOURCE and install it.
#   scripts/install.sh            native package for this distro (pacman/apt/dnf)
#   scripts/install.sh --user     binary + man page + completions into ~/.local
#   options: -y/--yes (no prompts), --no-deps (don't install build deps)
# Nothing prebuilt is installed: the binary is always compiled here with
# cargo build --release --locked (Rust >= MSRV below).
set -euo pipefail
MSRV=1.88
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
name=acp-red-star
user=0 yes=0 deps=1
for a in "$@"; do
  case $a in
    --user) user=1 ;;
    -y|--yes) yes=1 ;;
    --no-deps) deps=0 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done

say() { echo "install: $*" >&2; }
ask() {  # ask "question" -> 0 if yes
  [ $yes = 1 ] && return 0
  [ -t 0 ] || { say "$1 -> refusing without a terminal (pass --yes)"; exit 1; }
  read -r -p "$1 [y/N] " r; [[ $r =~ ^[Yy] ]]
}
SUDO=; [ "$(id -u)" = 0 ] || SUDO=sudo

# ---- distro detection (ID, then ID_LIKE)
family=other
if [ -r /etc/os-release ]; then
  . /etc/os-release
  for id in ${ID:-} ${ID_LIKE:-}; do
    case $id in
      arch|manjaro|endeavouros|garuda|artix) family=arch; break ;;
      debian|ubuntu|linuxmint|mint|pop|elementary|zorin|kali|raspbian) family=debian; break ;;
      fedora|rhel|centos|rocky|almalinux|alma|nobara|ol) family=fedora; break ;;
    esac
  done
fi
say "distro: ${PRETTY_NAME:-unknown} -> $family"

# ---- Rust toolchain (prefer rustup; distro rust may be too old)
[ -d "$HOME/.cargo/bin" ] && export PATH="$HOME/.cargo/bin:$PATH"
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }
rust_ok() {
  command -v cargo >/dev/null && command -v rustc >/dev/null || return 1
  local v; v=$(rustc --version | awk '{print $2}')
  ver_ge "$v" "$MSRV" || { say "rustc $v is older than $MSRV"; return 1; }
}
pm_install() {
  case $family in
    arch)   $SUDO pacman -S --needed --noconfirm "$@" ;;
    debian) $SUDO apt-get update -qq && $SUDO apt-get install -y "$@" ;;
    fedora) $SUDO dnf install -y "$@" ;;
    *) say "don't know how to install: $*"; return 1 ;;
  esac
}
pm_has() {
  case $family in
    arch) pacman -T "$1" >/dev/null 2>&1 ;;
    debian) dpkg -s "$1" >/dev/null 2>&1 ;;
    fedora) rpm -q --whatprovides "$1" >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}
ensure_rust() {
  rust_ok && { say "using $(rustc --version)"; return; }
  if command -v rustup >/dev/null; then
    ask "Install/update the stable Rust toolchain with rustup?" || exit 1
    rustup toolchain install stable && rustup default stable
  elif [ $deps = 1 ] && case $family in arch|fedora) true ;; debian) apt-cache show rustup >/dev/null 2>&1 ;; *) false ;; esac; then
    ask "Install the distro 'rustup' package and the stable toolchain?" || exit 1
    pm_install rustup
    command -v rustup >/dev/null || rustup-init -y --no-modify-path
    export PATH="$HOME/.cargo/bin:$PATH"
    rustup toolchain install stable && rustup default stable
  else
    ask "Download and run the official rustup-init from https://sh.rustup.rs?" || exit 1
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  rust_ok || { say "no usable Rust >= $MSRV"; exit 1; }
}

# ---- C toolchain + packaging tools
ensure_build_deps() {
  [ $deps = 1 ] || return 0
  local pk
  case $family in
    arch)   pk=(base-devel git) ;;
    debian) pk=(build-essential pkg-config); [ $user = 1 ] || pk+=(devscripts debhelper dpkg-dev) ;;
    fedora) pk=(gcc pkgconf-pkg-config); [ $user = 1 ] || pk+=(rpm-build rpmdevtools tar gzip) ;;
    *) command -v cc >/dev/null || { say "install a C compiler (gcc) and pkg-config first"; exit 1; }; return 0 ;;
  esac
  local missing=(); for p in "${pk[@]}"; do pm_has "$p" || missing+=("$p"); done
  [ $family = arch ] && { pacman -Qg base-devel >/dev/null 2>&1 && missing=("${missing[@]/base-devel}"); }
  missing=(${missing[@]+"${missing[@]}"})
  [ ${#missing[@]} = 0 ] && return 0
  ask "Install build dependencies: ${missing[*]}?" || exit 1
  pm_install "${missing[@]}"
}

ensure_build_deps
ensure_rust

# Does the package manager know about a cargo new enough? If not (rustup-only),
# skip the native build-dependency check.
nodeps=
if [ $user = 0 ]; then
  case $family in
    arch)   pacman -T 'cargo>=1.88' >/dev/null 2>&1 || nodeps=--nodeps ;;
    debian) dpkg --compare-versions "$(dpkg-query -W -f='${Version}' cargo 2>/dev/null || echo 0)" ge 1.88 || nodeps=--nodeps ;;
    fedora) rpm -q --qf '%{VERSION}\n' cargo 2>/dev/null | { read -r v || v=0; ver_ge "$v" 1.88; } || nodeps=--nodeps ;;
  esac
  [ -n "$nodeps" ] && say "distro cargo missing/older than $MSRV: building with the rustup toolchain and skipping the build-deps check"
fi

if [ $user = 1 ] || [ $family = other ]; then
  cargo build --release --locked
  if [ $user = 1 ]; then pfx="$HOME/.local"; S=; else pfx=/usr/local; S=$SUDO; fi
  [ $user = 1 ] || ask "Install into $pfx with sudo?" || exit 1
  stage=$(mktemp -d)
  scripts/stage.sh "$stage" target/release/$name /
  man=${pfx}/share/man/man1
  manifest="$pfx/share/$name/install-manifest"
  $S install -d "$pfx/share/$name"
  (cd "$stage" && find . -type f | sed 's,^\./,,') | while read -r f; do
    $S install -Dm"$( [ "$f" = bin/$name ] && echo 755 || echo 644)" "$stage/$f" "$pfx/$f"
    echo "$pfx/$f"
  done | $S tee "$manifest" >/dev/null
  echo "$manifest" | $S tee -a "$manifest" >/dev/null
  rm -rf "$stage"
  say "installed to $pfx (manifest: $manifest); man page in $man"
  case ":$PATH:" in *":$pfx/bin:"*) ;; *) say "add $pfx/bin to your PATH" ;; esac
  exit 0
fi

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
case $family in
  arch)
    (cd packaging/arch && makepkg -sfi $nodeps $([ $yes = 1 ] && echo --noconfirm)) ;;
  debian)
    scripts/package.sh deb $nodeps
    ask "Install dist/${name}_${ver}_*.deb with apt?" || exit 1
    $SUDO apt-get install -y "$root"/dist/${name}_${ver}_*.deb ;;
  fedora)
    scripts/package.sh rpm $nodeps
    ask "Install the built RPM with dnf?" || exit 1
    $SUDO dnf install -y $(ls "$root"/dist/$name-$ver-*.rpm | grep -v '\.src\.rpm$') ;;
esac
say "done: $(command -v $name || echo /usr/bin/$name) — uninstall with scripts/uninstall.sh"
