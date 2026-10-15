# Installing acp-rs-scraper

Every install method **builds from source** on your machine (`cargo build --release --locked`). No prebuilt binaries are shipped or installed.

- [Quick install](#quick-install)
- [Rust toolchain](#rust-toolchain)
- [Build dependencies](#build-dependencies)
- [Arch / Manjaro / EndeavourOS](#arch--manjaro--endeavouros)
- [Debian / Ubuntu / Mint / Pop!_OS](#debian--ubuntu--mint--pop_os)
- [Fedora / RHEL / CentOS / Rocky / Alma / Nobara](#fedora--rhel--centos--rocky--alma--nobara)
- [User install without a package (`--user`)](#user-install-without-a-package---user)
- [Building every package: `scripts/package.sh`](#building-every-package-scriptspackagesh)
- [Uninstalling](#uninstalling)
- [What gets installed](#what-gets-installed)

## Quick install

```bash
git clone https://forgejo.acp.us/m3talsmith/acp-rs-scraper
cd acp-rs-scraper
scripts/install.sh          # native package for your distro (asks before using sudo)
scripts/install.sh --user   # or: into ~/.local, no root needed
```

`install.sh` detects the distro from `ID` and then `ID_LIKE` in `/etc/os-release`:

| Family | IDs | Package manager | Built with |
|---|---|---|---|
| arch | arch, manjaro, endeavouros, garuda, artix | pacman | `makepkg -si` (packaging/arch/PKGBUILD) |
| debian | debian, ubuntu, linuxmint, pop, elementary, zorin, kali, raspbian | apt | `dpkg-buildpackage` (debian/), then `apt install ./dist/*.deb` |
| fedora | fedora, rhel, centos, rocky, almalinux, nobara, ol | dnf | `rpmbuild -ba` (packaging/fedora/acp-rs-scraper.spec), then `dnf install` |
| other | anything else | — | `cargo build`, binary installed to `/usr/local` (with sudo) |

**Options:**
- `--user`: install the locally built binary, man page and completions into `~/.local`.
- `-y`, `--yes`: answer yes to every prompt. Without it, a non-interactive stdin is refused.
- `--no-deps`: don't install build dependencies.

Every package-manager or download step is confirmed first.

## Rust toolchain

The crate declares `rust-version = "1.88"`, so **Rust ≥ 1.88** is required. Edition 2024 alone needs 1.85, but the `scraper` dependency uses let-chains, which became stable in 1.88; a 1.85 toolchain fails with `E0658`. This was tested: Debian 13's rustc 1.85 fails and `rust:1.88` builds.

Distro Rust is therefore **too old on Debian 13 (trixie, rustc 1.85)** and older Debian/Ubuntu releases; use rustup there. Arch and current Fedora ship new enough Rust.

`install.sh` looks for `cargo`/`rustc` on `PATH` and in `~/.cargo/bin` (rustup) and checks the version. If Rust is missing or too old, it prefers **rustup** over distro Rust packages, which may be too old on some releases:

1. **rustup already installed:** `rustup toolchain install stable`.
2. **No rustup:** offers the distro `rustup` package. This is available on Arch, on Fedora (then `rustup-init`), and on Debian 13+/testing when `apt-cache show rustup` finds it.
3. **Otherwise:** offers the official installer, `curl https://sh.rustup.rs | sh`, after confirmation.

**Toolchains the package manager can't see:** when your Rust comes from rustup, pacman/apt/dnf don't know about it, so the package build-dependency check would fail. `install.sh` detects this and passes the skip flag for you. If you build by hand, use:

| Build | Skip the build-dependency check with |
|---|---|
| Arch | `makepkg --nodeps` (`-d`) |
| Debian | `dpkg-buildpackage -d` |
| Fedora | `rpmbuild --nodeps` |
| `scripts/package.sh` | `--nodeps` |

**Package metadata:** the packages declare `makedepends=(cargo)` (Arch), `BuildRequires: cargo >= 1.88, rust >= 1.88` (Fedora) and `Build-Depends: cargo (>= 1.88), rustc (>= 1.88)` (Debian).

## Build dependencies

| Family | C toolchain | Packaging tools (native install only) |
|---|---|---|
| arch | `base-devel` (gcc, make, pkgconf, fakeroot) | `makepkg` (part of pacman) |
| debian | `build-essential pkg-config` | `devscripts debhelper dpkg-dev` |
| fedora | `gcc pkgconf-pkg-config` | `rpm-build rpmdevtools` |

SQLite is compiled in (rusqlite `bundled`) and TLS uses rustls, so no OpenSSL or libsqlite development packages are needed. Cargo downloads crates from crates.io during the build, so you need network access. Offline builders such as mock or koji would need a vendored tree.

## Arch / Manjaro / EndeavourOS

```bash
cd packaging/arch
makepkg -si          # build from this source tree, install with pacman
sudo pacman -R acp-rs-scraper
```

The PKGBUILD builds the tree it lives in (`packaging/arch/../..`), runs `cargo test`, and installs through `scripts/stage.sh`. A commented `source=` line shows how to build from a release tarball instead.

## Debian / Ubuntu / Mint / Pop!_OS

```bash
scripts/package.sh deb                       # dpkg-buildpackage on a clean copy → dist/
sudo apt install ./dist/acp-rs-scraper_1.12.0_*.deb
sudo apt remove acp-rs-scraper
```

`debian/` is a native (`3.0 (native)`) source package. `debian/rules` runs `cargo build --release`, `cargo test` (skipped with `DEB_BUILD_OPTIONS=nocheck`) and `scripts/stage.sh`. The source package (`.dsc` + `.tar.xz`) also lands in `dist/`.

## Fedora / RHEL / CentOS / Rocky / Alma / Nobara

```bash
scripts/package.sh rpm        # tarball of the tree + rpmbuild -ba → dist/*.rpm and *.src.rpm
sudo dnf install ./dist/acp-rs-scraper-1.12.0-1.*.x86_64.rpm
sudo dnf remove acp-rs-scraper
```

You can also rebuild the SRPM anywhere with `rpmbuild --rebuild dist/acp-rs-scraper-1.12.0-1.*.src.rpm`.

## User install without a package (`--user`)

```bash
scripts/install.sh --user
```

This builds with `cargo build --release --locked` and installs:
- `~/.local/bin/acp-rs-scraper`
- the man page under `~/.local/share/man/man1/`
- shell completions under `~/.local/share/{bash-completion,zsh,fish}`
- the docs under `~/.local/share/doc/acp-rs-scraper/`

It records every installed file in `~/.local/share/acp-rs-scraper/install-manifest`, which `uninstall.sh` uses. Make sure `~/.local/bin` is on your `PATH`. On distros the script doesn't recognise, an install without `--user` goes to `/usr/local` the same way.

## Building every package: `scripts/package.sh`

```bash
scripts/package.sh                 # deb rpm arch
scripts/package.sh deb rpm         # just these
scripts/package.sh all --container # force container builds
scripts/package.sh arch --nodeps   # rustup toolchain on the host
```

Each target uses the host tool (`dpkg-buildpackage`, `rpmbuild`, `makepkg`) when it is installed. If the tool is missing, the target is built inside a `debian:13`, `fedora:latest` or `archlinux:latest` container with podman or docker, using that distro's own Rust (Arch, Fedora), or rustup from the Debian `rustup` package for the Debian build, since trixie's Rust is too old. If no container engine works, the target is skipped and reported.

Builds use a clean copy of the tree (`target/`, `dist/` and `.git` excluded, uncommitted changes included). Results land in `dist/`, which is git-ignored.

## Uninstalling

```bash
scripts/uninstall.sh              # remove the package AND purge ~/.acp data (asks first)
scripts/uninstall.sh --no-purge   # remove the package, keep ~/.acp intact
scripts/uninstall.sh --yes        # no prompt (required when stdin isn't a terminal)
```

**Purging is the default.** The script finds how the tool was installed: a pacman, apt or dnf package, or a `--user`/`/usr/local` manifest. It then shows **one confirmation listing everything it will delete**, with sizes, and requires typing `yes`:
- the package, or the files listed in the manifest;
- `~/.acp/ai/data/rs.database` (plus `-wal`/`-shm`/`-journal` sidecar files);
- `~/.acp/ai/data/attachments/`;
- `~/.acp/ai/data/exports/`;
- `~/.acp/preferences.toml` (your saved email).

Empty `~/.acp/ai/data`, `~/.acp/ai` and `~/.acp` directories are removed afterwards. Any other files under `~/.acp` are left alone.

**Other options:**
- `--purge` is accepted as a no-op alias.
- Run under `sudo`, the script purges the invoking user's (`$SUDO_USER`) home, not root's.
- Without `--yes`, the script refuses to run when stdin isn't a terminal.

> **Package-manager removal alone never deletes your data.** `pacman -R`, `apt remove`/`purge` and `dnf remove` only remove the files the package installed. Package maintainer scripts can't, and deliberately don't, touch per-user home directories. To delete scraped data, use `scripts/uninstall.sh`, or from inside the tool, `acp-rs-scraper --clear-all` (which keeps preferences and exports).

## What gets installed

| Path | Contents |
|---|---|
| `/usr/bin/acp-rs-scraper` | the binary |
| `/usr/share/man/man1/acp-rs-scraper.1` | man page (`man acp-rs-scraper`) |
| `/usr/share/bash-completion/completions/acp-rs-scraper` | bash completion |
| `/usr/share/zsh/site-functions/_acp-rs-scraper` | zsh completion |
| `/usr/share/fish/vendor_completions.d/acp-rs-scraper.fish` | fish completion |
| `/usr/share/doc/acp-rs-scraper/` | README.md, USAGE.md, INSTALL.md, CHANGELOG.md (Debian gzips them and installs the changelog as `changelog.gz`) |
| `/usr/share/licenses/acp-rs-scraper/LICENSE` | GPL-3.0-or-later (Debian also ships `copyright` in the doc dir) |

The man page and completions come from the binary itself (a hidden `--generate man|bash|zsh|fish` flag), via `scripts/stage.sh`, which all three package builds share.
