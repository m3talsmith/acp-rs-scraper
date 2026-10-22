# Installing acp-red-star

Source install builds on your machine with [`scripts/install.sh`](../scripts/install.sh) (`cargo build --release --locked`).

Published binaries and alphas, once a release includes them, are on the [GitHub Releases](https://github.com/m3talsmith/acp-rs-scraper/releases) page for this repository. Docs link that page. An asset URL is written only after the file is published. Alpha packaging waits until writes and replies with attachments are done and the decentralized network work is completed. The server (`acp-rs`, `--serve`) and the relay (`acp-rs-relay`) each get a separate install and also stay available inside the app. Those packages wait for the same gate. That gate, and the checklist, are in [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.5. This document does not publish an alpha, a server package, or a relay package.

- [Quick install](#quick-install)
- [Workspace crates](#workspace-crates)
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
git clone https://github.com/m3talsmith/acp-rs-scraper
cd acp-red-star
scripts/install.sh          # native package for your distro (asks before using sudo)
scripts/install.sh --user   # or: into ~/.local, no root needed
```

The install puts three commands on `PATH`: `acp-red-star`, `acprs`, and `acp-rs-scraper`. `acprs` is the same program. `cargo install --path .` installs that short command too, because cargo cannot create a symlink. `acprs --version` prints `acp-red-star` and the version. `acprs --help` uses `acprs` in the usage line, the same way `acp-rs-scraper --help` uses that name. `acp-rs-scraper` prints one deprecation line through version 3. `acprs` does not.

`install.sh` detects the distro from `ID` and then `ID_LIKE` in `/etc/os-release`:

| Family | IDs | Package manager | Built with |
|---|---|---|---|
| arch | arch, manjaro, endeavouros, garuda, artix | pacman | `makepkg -si` (packaging/arch/PKGBUILD) |
| debian | debian, ubuntu, linuxmint, pop, elementary, zorin, kali, raspbian | apt | `dpkg-buildpackage` (debian/), then `apt install ./dist/*.deb` |
| fedora | fedora, rhel, centos, rocky, almalinux, nobara, ol | dnf | `rpmbuild -ba` (packaging/fedora/acp-red-star.spec), then `dnf install` |
| other | anything else | — | `cargo build`, binary installed to `/usr/local` (with sudo) |

**Options:**
- `--user`: install the locally built binary, man page and completions into `~/.local`.
- `-y`, `--yes`: answer yes to every prompt. Without it, a non-interactive stdin is refused.
- `--no-deps`: don't install build dependencies.

Every package-manager or download step is confirmed first.

## Workspace crates

The root `Cargo.toml` workspace members are `"."`, `"crates/acp_red_star_node_core"`, `"crates/acp_red_star_service_redstar"`, and `"crates/acp_red_star_service_youtube"`. These are the crates in the tree. Later names in the plan (`acp-rs-relay`, `acp-rs-tray`, `acp_rs_node_ffi`) are not members yet. X is not a member of this version. The YouTube crate plans a yt-dlp argument list and refuses the read. It does not spawn yt-dlp and it does not fetch.

| Package | Path | Version in this tree | What it is |
|---|---|---|---|
| `acp-red-star` | repository root (`Cargo.toml`) | 3.52.0 | Terminal client and CLI. The binaries are `acp-red-star`, `acprs`, and `acp-rs-scraper` (the compatibility name through version 3). The Cargo description is "ACP Red Star, a local client for the Red Star forum". Edition 2024, `rust-version` 1.88. |
| `acp_red_star_node_core` | `crates/acp_red_star_node_core` | 3.3.0 | Shared identity, canonical CBOR, sealed secrets, and the service plugin ABI. `publish = false`. The root package depends on it with `path = "crates/acp_red_star_node_core"`. It does not open the SQLite database, draw the terminal, or talk to the network. |
| `acp_red_star_service_redstar` | `crates/acp_red_star_service_redstar` | 3.52.0 | Red Star's capability set, settings schema, and `plugin_declaration`. The scrape, login, and write modules stay in the root crate. |
| `acp_red_star_service_youtube` | `crates/acp_red_star_service_youtube` | 3.52.0 | YouTube's capability set, settings schema, and `plugin_declaration`. Reads are prepared and gated. The crate does not spawn yt-dlp and does not open a connection. |

Public modules of `acp_red_star_node_core`: `cbor`, `hpke`, `master`, `node`, `password`, `service`, `slip39`, `stage`, `store`. The encoding rules are in [PROTOCOL.md](PROTOCOL.md).

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

SQLCipher is compiled in, with its OpenSSL vendored, and TLS uses rustls, so no system OpenSSL or libsqlite development packages are needed. Cargo downloads crates from crates.io during the build, so you need network access. Offline builders such as mock or koji would need a vendored tree.

## Arch / Manjaro / EndeavourOS

```bash
cd packaging/arch
makepkg -si          # build from this source tree, install with pacman
sudo pacman -R acp-red-star
```

The PKGBUILD builds the tree it lives in (`packaging/arch/../..`), runs `cargo test`, and installs through `scripts/stage.sh`. A commented `source=` line shows how to build from a release tarball instead.

## Debian / Ubuntu / Mint / Pop!_OS

```bash
scripts/package.sh deb                       # dpkg-buildpackage on a clean copy → dist/
sudo apt install ./dist/acp-red-star_1.25.0_*.deb
sudo apt remove acp-red-star
```

`debian/` is a native (`3.0 (native)`) source package. `debian/rules` runs `cargo build --release`, `cargo test` (skipped with `DEB_BUILD_OPTIONS=nocheck`) and `scripts/stage.sh`. The source package (`.dsc` + `.tar.xz`) also lands in `dist/`.

## Fedora / RHEL / CentOS / Rocky / Alma / Nobara

```bash
scripts/package.sh rpm        # tarball of the tree + rpmbuild -ba → dist/*.rpm and *.src.rpm
sudo dnf install ./dist/acp-red-star-1.25.0-1.*.x86_64.rpm
sudo dnf remove acp-red-star
```

You can also rebuild the SRPM anywhere with `rpmbuild --rebuild dist/acp-red-star-1.25.0-1.*.src.rpm`.

## User install without a package (`--user`)

```bash
scripts/install.sh --user
```

This builds with `cargo build --release --locked` and installs:
- `~/.local/bin/acp-red-star`, `~/.local/bin/acprs`, and `~/.local/bin/acp-rs-scraper`
- man pages for `acp-red-star` and `acprs` under `~/.local/share/man/man1/`
- shell completions for both commands under `~/.local/share/{bash-completion,zsh,fish}`
- the docs under `~/.local/share/doc/acp-red-star/`

It records every installed file in `~/.local/share/acp-red-star/install-manifest`, which `uninstall.sh` uses. Make sure `~/.local/bin` is on your `PATH`. On distros the script doesn't recognise, an install without `--user` goes to `/usr/local` the same way.

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
- the package, or the files listed in the manifest, including `acp-red-star`, `acprs`, `acp-rs-scraper`, and the `acprs` man page and completions;
- `~/.acp/ai/data/rs.database` (plus `rs.database.bootstrap` and `-wal`/`-shm`/`-journal` sidecar files);
- `~/.acp/ai/data/attachments/`;
- `~/.acp/ai/data/exports/`;
- `~/.acp/preferences.toml` (your saved email).
- `~/.acp/session.json` (your saved login session; `--logout` removes it).
- `~/.acp/recovery.seal` (the encrypted recovery phrase) and `~/.acp/recovery-next`.

Empty `~/.acp/ai/data`, `~/.acp/ai` and `~/.acp` directories are removed afterwards. Any other files under `~/.acp` are left alone.

**Other options:**
- `--purge` is accepted as a no-op alias.
- Run under `sudo`, the script purges the invoking user's (`$SUDO_USER`) home, not root's.
- Without `--yes`, the script refuses to run when stdin isn't a terminal.

> **Package-manager removal alone never deletes your data.** `pacman -R`, `apt remove`/`purge` and `dnf remove` only remove the files the package installed. Package maintainer scripts can't, and deliberately don't, touch per-user home directories. To delete scraped data, use `scripts/uninstall.sh`, or from inside the tool, `acp-red-star --clear-all` (which keeps preferences and exports).

## What gets installed

| Path | Contents |
|---|---|
| `/usr/bin/acp-red-star` | the binary |
| `/usr/bin/acprs` | the same program, short command |
| `/usr/bin/acp-rs-scraper` | the old name, through version 3 |
| `/usr/share/man/man1/acp-red-star.1` | man page (`man acp-red-star`) |
| `/usr/share/man/man1/acprs.1` | man page for the short command (`man acprs`) |
| `/usr/share/bash-completion/completions/acp-red-star` | bash completion |
| `/usr/share/bash-completion/completions/acprs` | bash completion for `acprs` |
| `/usr/share/zsh/site-functions/_acp-red-star` | zsh completion |
| `/usr/share/zsh/site-functions/_acprs` | zsh completion for `acprs` |
| `/usr/share/fish/vendor_completions.d/acp-red-star.fish` | fish completion |
| `/usr/share/fish/vendor_completions.d/acprs.fish` | fish completion for `acprs` |
| `/usr/share/doc/acp-red-star/` | README.md, USAGE.md, INSTALL.md, CHANGELOG.md (Debian gzips them and installs the changelog as `changelog.gz`) |
| `/usr/share/licenses/acp-red-star/LICENSE` | GPL-3.0-or-later (Debian also ships `copyright` in the doc dir) |

The man pages and completions come from the binary itself (a hidden `--generate man|bash|zsh|fish` flag, and the same values prefixed `acprs-`), via `scripts/stage.sh`, which all three package builds share. Uninstall removes `acprs` with the rest of those files.
