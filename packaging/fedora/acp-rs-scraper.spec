# Build from source: scripts/package.sh rpm   (or rpmbuild -ba with Source0 =
# a tarball of the source tree, top directory acp-rs-scraper-%{version}/).
# Uninstall: sudo dnf remove acp-rs-scraper
# Rustup users whose distro cargo is missing/too old: rpmbuild --nodeps.
# Crates are fetched from crates.io during %build, so offline builders
# (mock/koji) need network or a vendored tree.
%global debug_package %{nil}

Name:           acp-rs-scraper
Version:        1.26.0
Release:        1%{?dist}
Summary:        Scrape Red Star (acp.us/rs) posts into SQLite and export them
License:        GPL-3.0-or-later
URL:            https://github.com/m3talsmith/acp-rs-scraper
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  gcc

%description
Logs in to https://acp.us/rs, scrapes posts, mentions and profiles for one or
more nicks into a local SQLite database, downloads attachments, and exports
JSON/HTML. Search, export, import and maintenance work offline.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked

%check
cargo test --release --locked

%install
scripts/stage.sh %{buildroot} target/release/%{name} %{_prefix}

%files
%license %{_datadir}/licenses/%{name}/LICENSE
%doc %{_docdir}/%{name}/
%{_bindir}/%{name}
%{_mandir}/man1/%{name}.1*
%{_datadir}/bash-completion/completions/%{name}
%{_datadir}/zsh/site-functions/_%{name}
%{_datadir}/fish/vendor_completions.d/%{name}.fish

%changelog
* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 1.26.0-1
- Complete truncated post/comment bodies from thread pages; keep paragraph breaks; --refetch-truncated

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 1.25.1-1
- Exports mark posts vs replies and list posts first (--said-by fix)

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.25.0-1
- --format accepts several formats (json,html) in one run

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.24.0-1
- Redact CSRF/session secrets in --debug logs; fail on unknown nicks; never write empty exports

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.16.0-1
- Find profile photos for reply-only members; never use the site banner

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.15.0-1
- Post bodies in every site shape, profile images, compressed-export cleanup

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.12.0-1
- Cleanup release, Gherkin e2e suite, Posts and replies filter, fresh-HOME DB fix

* Thu Oct 15 2026 m3talsmith <m3talsmith@gmail.com> - 1.4.0-1
- First package: build from source, man page and shell completions
