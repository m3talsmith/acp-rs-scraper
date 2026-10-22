# Build from source: scripts/package.sh rpm   (or rpmbuild -ba with Source0 =
# a tarball of the source tree, top directory acp-red-star-%{version}/).
# Uninstall: sudo dnf remove acp-red-star
# Rustup users whose distro cargo is missing/too old: rpmbuild --nodeps.
# Crates are fetched from crates.io during %build, so offline builders
# (mock/koji) need network or a vendored tree.
%global debug_package %{nil}

Name:           acp-red-star
Version:        3.35.14
Release:        1%{?dist}
Summary:        Scrape Red Star (acp.us/rs) posts into SQLite and export them
License:        GPL-3.0-or-later
URL:            https://github.com/m3talsmith/acp-rs-scraper
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  gcc
Recommends:     (libavif-tools or ImageMagick or ffmpeg-free)
Recommends:     xdg-utils
Suggests:       python3, yt-dlp

%description
Logs in to https://acp.us/rs, scrapes posts, mentions and profiles for one or
more nicks into a local SQLite database, downloads attachments, and exports
JSON/HTML. Search, export, import and maintenance work offline.
Run with no flags for a terminal UI that browses the database and drives scans.

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
%{_bindir}/acp-rs-scraper
%{_bindir}/acprs
%{_datadir}/%{name}/
%{_mandir}/man1/%{name}.1*
%{_mandir}/man1/acprs.1*
%{_datadir}/bash-completion/completions/%{name}
%{_datadir}/bash-completion/completions/acprs
%{_datadir}/zsh/site-functions/_%{name}
%{_datadir}/zsh/site-functions/_acprs
%{_datadir}/fish/vendor_completions.d/%{name}.fish
%{_datadir}/fish/vendor_completions.d/acprs.fish

%changelog
* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.14-1
- Package 3.35.14. A pending recovery phrase is asked in the terminal UI. Not now is the default.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.13-1
- Package 3.35.13. A second basket export leaves the earlier file and writes a new name.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.12-1
- Package 3.35.12. Research add file shows ~ or a relative path.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.11-1
- Package 3.35.11. HTML export renders Markdown. Raw HTML in a post is escaped.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.10-1
- Package 3.35.10. Help search finds the layout topic.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.9-1
- Package 3.35.9. Question mark opens the key list. The recovery phrase is its own Help topic.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.8-1
- Package 3.35.8. A neutral brand leaves out the ACP wording. The ACP brand keeps it.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.7-1
- Package 3.35.7. Thread sort reorders stored replies and does not open login.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.6-1
- Package 3.35.6. Help search keeps j and k.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.5-1
- Package 3.35.5. The theme editor preview uses the brand on screen and the sample nicks.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.4-1
- Package 3.35.4. Theme editor samples use ordinary preview text. Explore stops after 12 seconds with no reply.

* Thu Oct 22 2026 m3talsmith <m3talsmith@gmail.com> - 3.35.0-1
- Package 3.35.0. The logo, title, and motto are a brand. --brand plain is a neutral title and a star, with no motto.

* Wed Oct 21 2026 m3talsmith <m3talsmith@gmail.com> - 3.34.0-1
- Package 3.34.0. Drafts and the outbox. A queued text post or reply uses the same confirm rules as a direct publish.

* Wed Oct 21 2026 m3talsmith <m3talsmith@gmail.com> - 3.33.0-1
- Package 3.33.0. A reply edit sends validate, then save, once. The site does not support changing attachments on an edit.

* Wed Oct 21 2026 m3talsmith <m3talsmith@gmail.com> - 3.32.0-1
- Package 3.32.0. Versions 3.6.0 through 3.31.0 are in CHANGELOG.md and are not separate package releases.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.5.0-1
- Attach local media and pdf files on New post and Reply. Dry-run lists them. Upload stays unsent.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.4.0-1
- New post and Reply. Publish stays a local preview until a guided capture.
- Includes the 3.3.2 certificate lifetimes (authorizer 30 days, node 365 days).

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.3.2-1
- Authorizer certificates last 30 days and node certificates last 365 days.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.3.1-1
- Job details: elapsed time stops when the job finishes, fails, or is cancelled.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.3.0-1
- Node identity: a local key, certificates, --node-init, and --node-id.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.2.0-1
- Shared core crate for identity, canonical CBOR, and sealed secrets.

* Mon Oct 19 2026 m3talsmith <m3talsmith@gmail.com> - 3.1.0-1
- Job details: click the status bar or the header running mark

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 3.0.0-1
- Breaking: encrypted database, mandatory password, --encrypt-db migrates a 2.x file after a backup

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.24.1-1
- Bookmark sync uses the bookmark hook with an explicit state

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.24.0-1
- Edit history as a diff, stored in post_versions

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.23.0-1
- Reposts list (/rs/post/ID/reposts)

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.22.0-1
- Thread sort (oldest/top) and a score column

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.21.0-1
- Tag feeds, body_json and post_tags

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.20.0-1
- Explore feed and site search

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.19.0-1
- Executive Board feed (/rs/eb) with a replies filter

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.18.0-1
- Site-wide Top feed (/rs/top) with a period and a replies filter

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.17.0-1
- Full Markdown in post bodies with new theme elements; list bullets fixed

* Sun Oct 18 2026 m3talsmith <m3talsmith@gmail.com> - 2.16.0-1
- Theme editor sample dialog, per-row revert, save prompts, truecolor note; header missing/broken marker; grouped theme warnings.

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.15.0-1
- Theme editor preview highlight, starter themes, header download status

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.14.0-1
- Themes only from ~/.acp/themes; first-run default theme download; --theme-init

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.13.0-1
- Six communist themes with dark and light variants

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.12.0-1
- File-based TUI themes, theme picker and editor

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.11.1-1
- Taller TUI header and a recognisable red-star logo

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.11.0-1
- Empty-Body TUI animation and tools/acp-anim

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.10.0-1
- TUI styling moved to ratatui-lipgloss

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.9.0-1
- Research projects (--research-*) with a TUI picker and Research menu

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.8.0-1
- TUI export basket and --post-ids exports

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.7.0-1
- TUI inline pictures and 3-row header with logo and big ACP

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.6.0-1
- TUI Markdown bodies, padding and openable attachment links

* Sat Oct 17 2026 m3talsmith <m3talsmith@gmail.com> - 2.5.0-1
- TUI nick filter; bookmark sync confirmed against the live site

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 2.4.0-1
- TUI previous / next through the detail stack

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 2.3.0-1
- TUI view modes: stack, pane, carousels and grid

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 2.2.0-1
- --sync and automatic TUI sync with the site

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 2.1.0-1
- Bookmarks (local and synced with the site), TUI stars and bookmarks view

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 2.0.0-1
- Terminal UI by default (no flags or --tui); --scrape for the old bare scrape

* Fri Oct 16 2026 m3talsmith <m3talsmith@gmail.com> - 1.26.1-1
- Do not duplicate attachments when media links are re-signed

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
