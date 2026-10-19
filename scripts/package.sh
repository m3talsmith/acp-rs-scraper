#!/usr/bin/env bash
# Build native packages FROM SOURCE into dist/.
#   scripts/package.sh [deb|rpm|arch|all] [--container] [--nodeps]
# deb : debian/ source package via dpkg-buildpackage (binary + source)
# rpm : packaging/fedora/acp-red-star.spec via rpmbuild -ba (RPM + SRPM)
# arch: packaging/arch/PKGBUILD via makepkg (.pkg.tar.zst)
# Each target uses the host tools when present; otherwise (or with
# --container) it builds inside a debian:13 / fedora:latest / archlinux
# container with docker or podman. --nodeps skips the build-dependency check
# (for rustup toolchains the package manager doesn't know about).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
name=acp-red-star
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
dist="$root/dist"
targets=() failed=() container=0 nodeps=0
for a in "$@"; do
  case $a in
    deb|rpm|arch) targets+=("$a") ;;
    all) targets+=(deb rpm arch) ;;
    --container) container=1 ;;
    --nodeps) nodeps=1 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done
[ ${#targets[@]} -gt 0 ] || targets=(deb rpm arch)
mkdir -p "$dist"

# A clean copy of the source tree (no target/, dist/, .git), including
# uncommitted changes, as name-ver/ in a tarball.
src_tarball() {
  local out=$1
  tar --exclude=./target --exclude=./dist --exclude=./.git --exclude='./docs/api/.lock' \
      --transform "s,^\.,$name-$ver," -czf "$out" -C "$root" .
}

engine() { command -v podman >/dev/null && echo podman || { command -v docker >/dev/null && docker info >/dev/null 2>&1 && echo docker; } || true; }

in_container() {  # target image deps
  local t=$1 img=$2 deps=$3 eng; eng=$(engine)
  [ -n "$eng" ] || { echo "$t: no host tools and no working docker/podman; skipped" >&2; return 3; }
  echo "== $t: building in $img via $eng" >&2
  local work; work=$(mktemp -d)
  src_tarball "$work/src.tar.gz"
  chmod 755 "$work"; chmod 644 "$work/src.tar.gz"
  $eng run --rm --network host -v "$work:/in:Z" -v "$dist:/out:Z" "$img" bash -euc "
    $deps
    mkdir -p /build && tar -xzf /in/src.tar.gz -C /build
    cd /build/$name-$ver && scripts/package.sh $t \${PKG_NODEPS:-}
    cp dist/*.deb dist/*.dsc dist/*.tar.xz dist/*.rpm dist/*.pkg.tar.zst /out/ 2>/dev/null || true
    chown -R $(id -u):$(id -g) /out" || { rm -rf "$work"; return 1; }
  rm -rf "$work"
}

build_deb() {
  local b="$dist/build/deb"; rm -rf "$b"; mkdir -p "$b"
  src_tarball "$b/src.tar.gz"; tar -xzf "$b/src.tar.gz" -C "$b"
  (cd "$b/$name-$ver" && dpkg-buildpackage -us -uc $([ $nodeps = 1 ] && echo -d))
  cp "$b"/*.deb "$b"/*.dsc "$b"/*.tar.* "$dist/" 2>/dev/null || true
}

build_rpm() {
  local b="$dist/build/rpm"; rm -rf "$b"; mkdir -p "$b"/{SOURCES,SPECS,BUILD,RPMS,SRPMS}
  src_tarball "$b/SOURCES/$name-$ver.tar.gz"
  rpmbuild --define "_topdir $b" -ba packaging/fedora/$name.spec $([ $nodeps = 1 ] && echo --nodeps)
  cp "$b"/RPMS/*/*.rpm "$b"/SRPMS/*.rpm "$dist/"
}

build_arch() {
  local b="$dist/build/arch"; rm -rf "$b"; mkdir -p "$b"
  (cd packaging/arch && PKGDEST="$dist" BUILDDIR="$b" SRCDEST="$b" PKGEXT=.pkg.tar.zst \
     makepkg -f --noconfirm $([ $nodeps = 1 ] && echo --nodeps))
}

for t in "${targets[@]}"; do
  case $t in
    deb)  tool=dpkg-buildpackage img=docker.io/library/debian:13
          deps="export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get install -y -qq build-essential debhelper devscripts pkg-config rustup >/dev/null; rustup toolchain install --profile minimal stable 2>&1 | tail -1; rustup default stable; export PATH=\$HOME/.cargo/bin:\$PATH PKG_NODEPS=--nodeps" ;;
    rpm)  tool=rpmbuild img=registry.fedoraproject.org/fedora:latest
          deps="dnf -y -q install rpm-build rpmdevtools gcc pkgconf-pkg-config cargo rust tar gzip >/dev/null" ;;
    arch) tool=makepkg img=docker.io/library/archlinux:latest
          deps="pacman -Syu --noconfirm --needed base-devel rust git >/dev/null; useradd -m b; mkdir -p /build; chown b /build; su b -c 'set -e; tar -xzf /in/src.tar.gz -C /build; cd /build/$name-$ver; scripts/package.sh arch'; cp /build/$name-$ver/dist/*.pkg.tar.zst /out/; chown -R $(id -u):$(id -g) /out; exit 0" ;;
  esac
  if [ $container = 0 ] && command -v "$tool" >/dev/null && { [ "$t" != arch ] || [ "$(id -u)" != 0 ]; }; then
    echo "== $t: building on host with $tool" >&2
    "build_$t" || failed+=("$t")
  else
    in_container "$t" "$img" "$deps" || failed+=("$t")
  fi
done
echo "== artifacts in $dist:" >&2
ls -1 "$dist" | grep -E '\.(deb|rpm|pkg\.tar\.zst|dsc|tar\.xz)$' >&2 || echo "(none)" >&2
if [ ${#failed[@]} -gt 0 ]; then echo "== FAILED: ${failed[*]}" >&2; exit 1; fi
