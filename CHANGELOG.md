# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **`acprs`.** A third binary, next to `acp-red-star` and the `acp-rs-scraper` compatibility name. `cargo install` cannot create a symlink, so `acprs` is its own executable and runs the same program. `--version` still prints `acp-red-star` and the version. The usage line uses the name you typed, as `acp-rs-scraper --help` already does, and `acprs` does not print the deprecation line. Packages and `scripts/install.sh` install it with a man page and bash, zsh, and fish completions. `scripts/uninstall.sh` removes it with the rest of the install. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.
- **`rs` theme.** A shipped theme matching the Red Star site (acp.us/rs): near-black zinc `#111113`, text `#d3d3d7`, muted `#898994`, divider `#333336`, ACP red `#c01d1c` (the logo and links), and vote gold `#eaa313` for bold text, H3, and table headers. Selection, buttons, and the header use the site's grey surfaces (`#2b2b2e`, `#0b0b0b`), and quotes are muted text on the divider colour. The light variant is zinc grey with the same red and a darkened gold. `rs` is in the theme picker once the file is in `~/.acp/themes`; `--theme-init rs` downloads it. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.
- **`acp` theme.** A shipped theme matching the American Communist Party site (acp.us). The light variant is the site: blush `#fff1f1`, black `#000000`, ACP red `#c01c1c`, a white chip for buttons, and a pink-grey rule darkened from `#edc8c8` so it shows on the blush. Muted text is the map grey `#ccb3b3` darkened to `#706262` so it clears WCAG AA. H1 is white on red. The dark variant is black with blush text `#fff1f1`, the same red, and blush `#ffcdcd` for secondary text. There is no gold. `acp` is in the theme picker once the file is in `~/.acp/themes`; `--theme-init acp` downloads it. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.
- A few surprises for the curious.
- If you go looking, there is art in the dark of the tree.

### Changed

- **Notices.** An error, a warning, a skipped sync, or a finished job raises a stacked toast and a desktop notification, and the notice log keeps it. View › Notices opens that log. Do not disturb and a switch for each kind silence the toast and the desktop notification together. The log still records them. The status line keeps job and list state. The same notice prints on stderr and stays plain when colour is off. Tests use a fake desktop backend and do not talk to a notification daemon. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later, including 3.22.0 and the planned 3.31.0, are not moved.
- **Search history and the action bar.** Each local or site search is a row in the encrypted database: the query and the filters only, including order and limit. Picking one re-runs it. Clear history deletes those rows and leaves posts alone. The default keeps the entire history. Search › History kept can cap it at the N newest rows, stored as `[ui] search_history_limit` in `preferences.toml`. An export of a search adds a `search` object with those fields. Format version stays 2. A full dump, a nick filter, and a basket do not add one. The right bar lists every action that led to the current view, with no cap, and collapses to one column. The list scrolls with the wheel and, when the bar is focused, with the arrows, PgUp, and PgDn. Clicking an earlier action jumps back. A new action after a jump keeps the later steps as another branch; the bar shows which branch, and `b` lists them. Branches are stored in the encrypted database. Clear history deletes the searches and the branches. An export from a restored search still carries the query. The nick bar collapses the same way. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later, including 3.22.0, are not moved.
- **Menu bar categories.** The bar is View, Search, Compose, Library, Bookmarks, Research, Sync, Discover, Account, Debug, Help, About, and Quit. Scrape is pinned immediately left of Start Scanning, and its options stay on that control. Library opens Export, Import, Basket, List, and Maintain; those stay off the bar. Quit stays the last menu item. About stays beside Help. Discover says people, nodes, and rooms arrive with the network node. Search › Search the site runs `--site-search` with the Search query. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.
- **Tooltips, Help, and About.** Hover shows a short explanation on every button, menu entry, and card icon, including the key when the control has one (`Bookmark (b)`). The second line is `? {topic}` and opens that action's Help topic. Star, basket, and research do not share a topic. One helper draws every tooltip. Help is topics you can search and step through with ← →. `?` and F1 stay. A `?` in the header corner opens Help. Getting Started is `docs/GETTING-STARTED.md` and the `getting-started` topic, covering the command line, terminal prompts, and the TUI. About is a menu with three segments — Credits, Why this is necessary, and ACP platform — and each opens the same modal on that section. ACP platform is the vendored American Communist Party Program (adopted October 12, 2024), with headings and scroll. `https://acp.us/join` sits at the top and the bottom of that section only. Publish still does not send while `CAPTURE_BLOCK` is in force. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later, including 3.22.0, are not moved.
- **Clippy is clean.** `scrape` takes a `ScrapeOptions` value and `read_view` takes a `ViewRequest`, instead of a long argument list. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.
- **`red-star` theme.** The shipped theme matches the brand banner: ink `#0e0404`, cream `#f6e8c8`, vermilion `#c8102e`, gold `#e8b648`, with a light variant on cream. It replaces the old deep-crimson palette. The name stays `red-star`, so `--theme red-star` still selects it and the theme picker shows the new description. A copy already saved in `~/.acp/themes` is not overwritten. Delete `~/.acp/themes/red-star.toml` and run `--theme-init red-star` (or copy `themes/red-star.toml`) to take the new look. This is its own minor. It is not part of 3.5.0. The crate version stays 3.5.0 until that minor is released. 3.6.0–3.9.0 stay unassigned, and 3.10.0 and later are not moved.

### Fixed

- **Terminal UI login, follow-up.** A Phoenix heartbeat that is written but not answered within one interval is a dead socket, and the login reconnects without sending another email. The sign-in POST uses the code form from the LiveView diff: `user[otp]`, `submit-code`, that form's `_csrf_token` (not the page token), `user[remember_me]`, `marketing_opt_in` when the form has it, and `return_to` from the `/auth` URL. A redirect back to `/auth` is a rejection and the page's flash is the error. The code form was missed because the check looked for `user[code]` and `verify-code`, and a keyed comprehension whose statics live in the `p` template table was not rendered. The login window names the real session file, including when `ACP_HOME` is set. The one-time code is not written to the debug log.
- **Terminal UI login.** Entering the emailed code no longer fails with "websocket closed". The sign-in POST kept a non-`Send` HTML node alive across the request, so the terminal UI ran login on a second runtime and that split closed the LiveView socket when the code was submitted. The form fields are copied first and the login runs on the same runtime as the HTTP client, as the command line already does. A reconnect closes the old socket before opening another. If the site renders the sign-in form and then closes the socket, that form is still posted. The session is written to `~/.acp/session.json`, the same file the command line writes. `ACP_HOME`, when set and not empty, is the directory that holds `.acp` instead of `$HOME`, so a login can be tried without touching the real `~/.acp`.

### Documentation

- **Server and relay packaging is parked on the alpha gate.** The server (`acp-rs`, `--serve`) and the relay (`acp-rs-relay`) each get a separate install, with its own binary, release artifact, and install path, and each stays available inside the app. No server or relay release is cut until writes and replies with attachments are functional and the decentralized network work is functional. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.5 and §7.14.11.
- **Alpha packaging is parked.** An alpha waits until writes and replies with attachments are sent and the decentralized network work is completed. This note does not tag an alpha or attach release assets. Launch docs name `acp-red-star` and `acp_red_star_node_core`, keep `scripts/install.sh`, and point at the repository Releases page for binaries once they are published. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md).
- **Workspace crates and install.** Launch docs name the two crates in the workspace: `acp-red-star` at the repository root, and `acp_red_star_node_core` at `crates/acp_red_star_node_core`. Source install stays `scripts/install.sh`. The README and [docs/INSTALL.md](docs/INSTALL.md) also point at the [GitHub Releases](https://github.com/m3talsmith/acp-rs-scraper/releases) page for binaries and alphas once a release publishes them. Alpha packaging stays parked until writes and replies with attachments are done and the decentralized network work is completed. This note does not tag an alpha or attach release assets. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.5.
- **Attachments stay 3.5.0 (confirmed).** Michael confirmed the three decisions: `.jpeg` is accepted as jpg and `.tif` as tiff; there is no count cap and no size cap; attachments keep 3.5.0. Interactions were numbered 3.5.0–3.9.0 and have not started. 3.10.0 and later, including the locked 3.22.0 re-key, are not shifted. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md).
- **Draft GitHub Releases.** A version that is not on `main` is a draft release, with the notes from its changelog section and no attached files. The maintainer publishes the draft after that version is merged. Releases for versions already on `main` (2.16.0 and earlier) are left alone. The post-commit hook still only creates the git tag. Nothing in CI publishes a release.
- **Tombstones, revised.** A signed tombstone keeps the post. The plan no longer drops the body after a grace period. The UI shows a tombstone icon (🪦, or `[RIP]`), the content stays viewable, and a signed restore record or a newer signature of the same post id clears the tombstone. The restore record is in [docs/PROTOCOL.md](docs/PROTOCOL.md).
- **Search history.** The client keeps each local or site search so a past query can be opened again, as part of the Search menu. The minor is not numbered. A row in the encrypted database keeps the query and the filters only, not a snapshot of the results. The filters include order (`newest` or `oldest`) and limit (N, or none). Picking one re-runs the search live. Clear history deletes the rows. The default keeps the entire history. A preference can instead keep the N most recent entries. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §7.14.5.
- **Search export.** Exporting a search result includes the query and the filters that produced it, as a `search` object. Format version stays 2. A full dump, a nick filter, or a basket does not add a search. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §7.14.5.
- **Action history.** The client has a right bar that pulls out and collapses with every action that led to the current view. The list scrolls. Clicking an earlier action jumps back, and a new step after that jump keeps the later steps as another branch in the encrypted database. Clear history deletes those branches. An export from a search state still includes the query and filters. The left nick bar collapses the same way. This is not the previous and next post controls. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §7.14.5.
- **Embedded assistant.** A local model inside the client is a 4.0 roadmap item, not a 3.x requirement and not a heavy interest of 3.0. It answers questions about the local corpus and runs existing actions such as search, filter, and export. An AI button at the bottom center opens a chat strip that pushes the body and side bars up. It is not a modal. The strip keeps the chat, and the actions change the body. The voice is a local persona: a system prompt and optional retrieval first, and a later LoRA-style adaption on a local corpus if wanted. That is not training a model from scratch. Decentralization stays in 3.x. The model is not chosen. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §7.14.12.
- **Password change / re-key is 3.22.0.** It does not change the master or the `acp-id`. It re-wraps the SQLCipher key, the keystore, the sealed phrase, and the backup-wrapping key in one transaction, with rollback, and it requires the old password. It sits before the daemon and the OS keyring. Stages that were 3.22.0 through 3.28.0 are now 3.23.0 through 3.29.0. The write core stays 3.4.0.
- **4.0 roadmap, docs only.** Post source and the Haz stream corpus sit beside the embedded local assistant in `architecture/PLAN-3.0-CLIENT.md` (§4.0; post source is §7.14.14, the assistant is §7.14.12). The living plan is [architecture/PLAN-4.0.md](architecture/PLAN-4.0.md). Coming-soon and launch drafts are under `docs/history/v4/`. No schema change and no scraper in this note. The source columns may land later as a 3.x minor.
- **TUI menu categories.** Before 3.x is finished the menu bar is View, Search, Compose, Library, Bookmarks, Research, Sync, Discover, Account, and Debug, then Help, About, and Quit, with Scrape and Start Scanning pinned on the right. Bookmarks is its own menu (Show, Bookmark current, Sync with acp.us), not an item under Library. Search history joins Search in its own minor when Search work is touched. Sync keeps the local copy up to date. Discover finds people, nodes, and rooms. The two stay separate. The minor is required and is not numbered in this note. Quit stays the last menu item. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.2.
- **Hover tooltips.** Every button, menu item, menu-bar entry, and card or detail icon shows a short explanation on hover. A key is included when the control has one, as in `Bookmark (b)`. The second line, `? {topic}`, opens the Help topic for that action. One generic Help page for every tooltip does not count. One shared helper draws them. The minor is its own and is not numbered here. Publish stays behind `CAPTURE_BLOCK`. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.3.
- **Help topics.** Help is topics you can move between (Getting started, Browse, Compose, Research, Sync, Discover, Account, Keys, and the rest), with search inside Help. A `?` on the header opens Help, and every action tooltip deep-links to the topic for that particular action. `?` and F1 stay the entry. The minor is its own and is not numbered here. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.4.
- **Embedded assistant is 4.0.** It is not a 3.x requirement and is not a heavy interest of 3.0. 4.0 may treat it as a substantial part of that major. Decentralization stays in 3.x (3.23.0 and later). Opening a 4.0 line does not move peers, relays, or the network node.
- **Help coverage and Getting Started.** Every user-facing flag, menu command, menu option, and control has a Help description, and every action tooltip deep-links to the topic for that particular action. The walkthrough at `docs/GETTING-STARTED.md` covers the command line, terminal prompts outside the TUI, and the TUI, and it is the Getting started Help topic. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.4.
- **About.** About is a menu item with three segments: Credits, Why this is necessary, and ACP platform. Choosing any segment opens the same modal, focused on that section. ACP platform is the vendored American Communist Party Program (adopted October 12, 2024, <https://acp.us/program>), with headings and scroll. The same how-to-join link, <https://acp.us/join>, sits at the top and the bottom of that text only and is not a fourth section. Each segment has a tooltip and a Help topic, and the tooltip deep-links to that segment. Publish stays behind `CAPTURE_BLOCK`.
- **v3 pre-launch write-up, historical draft.** [docs/history/v3/redstar-v3-prelaunch-post.md](docs/history/v3/redstar-v3-prelaunch-post.md) keeps the comrades' coming-soon forum draft for version 3, including the launch showcase. It is not the live post. The README links it. At the 3.0 release, switch that link to `docs/history/v3/redstar-v3-launch-post.md`, the launch post written as if 3.x has fully shipped.
- **ACP Red Star and launch communications.** The user-facing name during 3.x is ACP Red Star. A docs pass (that name, Getting Started, Help coverage, About, `SECURITY.md`, and a refresh of SPECS and PLAN-2.0) comes before the client is presented as launched. After that pass: an elevator pitch, a CLI and TUI demo, a graphical sticker set, a separate CLI-ethos sticker set, ethos backgrounds, a cutscene in normal video and ASCII from public-domain or otherwise licensed archival footage, and a Red Star forum write-up for comrades covering v3. That post features the cutscene in both forms beside the pitch and the demo. A separate community-work video, chapter fieldwork rather than the cutscene or the demo, is also promo and is not produced here. The pre-launch / coming-soon draft is [docs/history/v3/redstar-v3-prelaunch-post.md](docs/history/v3/redstar-v3-prelaunch-post.md). It is not the live forum post. At the 3.0 release, the README link switches to `docs/history/v3/redstar-v3-launch-post.md`. The GitHub repo stays `m3talsmith/acp-rs-scraper`. The package and the shipping binary are `acp-red-star`. See [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) §4.5.
- **Research, add the current view.** Research › Add current view / search… already queues the current list or search. Making that easier to find is required before 3.x is finished. Whether "the whole Body" also means an open thread's root and its loaded replies is still open. No key is assigned.

## [3.5.0] - 2026-10-19

### Changed

- **The product name is ACP Red Star.** The binary is `acp-red-star`. `acp-rs-scraper` still runs, prints one deprecation line, and forwards its arguments through version 3. The library crate is `acp_red_star`. The shared core is `acp_red_star_node_core`. The encrypted database stays at `~/.acp/ai/data/rs.database`. `ACP_*` environment variables are unchanged. The GitHub repository URL is unchanged. The terminal UI shows the slogan "The future belongs to the working class" under that title on the welcome screen and the login screen, as the header of About, and at the right of the status bar when the bar is wide enough to leave the status text intact. With `NO_COLOR` or `--color never` the slogan is plain text.

### Added

- **`--color auto|always|never`.** `auto` (the default) follows `NO_COLOR`. `always` forces colours on. `never` turns them off. `[ui] color` in `preferences.toml` is the default when the flag is omitted. When `NO_COLOR` is set and colours are off, the terminal UI says so at start, when a theme is chosen or edited, and in the theme picker and theme editor. View › Colour sets the same choice.
- **`--limit N` on a nick scrape.** `acp-red-star --nick alice --limit 100` keeps the newest 100 posts from that profile (Posts, and Posts and replies). Omit it to scrape the whole profile. Replies in those threads are still fetched unless `--said-by`. `--crawl-limit` still only bounds the mentions scan. Start Scanning has the same field, Post limit, blank for the whole profile.
- **Order and limit in the terminal UI.** Every post list has an order, newest or oldest: nick timelines, bookmarks, site lists (Top, EB, Explore, a tag, reposts), search results, research items, and the export basket. `o` toggles it, the nav bar shows `[Order: newest ▸]`, and View › Order sets it. Each list remembers its choice under `[ui.list_order]` in `~/.acp/preferences.toml`. Nested view orders roots and keeps replies under the root, oldest first. Flat layouts order every card. Site lists keep the site's order (oldest reverses it; Top stays a ranking). Edit history stays in version order. Search and the nick filter also take a limit: N items at the chosen end, blank for the whole list, the same window as `--order` and `--limit`. The status line shows the window (`search "strike": 100 of 236 posts · oldest`). `?` on one of these menu options opens its own Help topic.
- **`--profile-feed` and `--order` on a nick scrape.** `--profile-feed newest` (the default) is the Latest profile. `--profile-feed top` is `/rs/@nick/top?t=all` only; other Top periods were not captured for profiles, and Posts and replies is not read on that feed. `--order newest` (the default) keeps the start of that list. `--order oldest` keeps the end, so `--limit` is the oldest N on the Latest profile, or the end of the Top ranking. `--latest` / `--trending` still only choose the mentions crawl. Start Scanning has Profile feed and Order. The Sync menu does not.
- **Attachments on New post and Reply.** The compose box has an attach line: type a path and Enter. Tab on a non-empty line completes the next real path segment (`~` becomes `~/`, and `~/…` stays in that form). Several matches are listed; Tab, Up, and Down move through them, and Enter writes the highlighted one without attaching yet. An empty line, or a Tab that changes nothing, moves focus. Backspace on an empty line removes the last file. `--attachment PATH` repeats on `--new-post` and `--reply`; the shell completes that flag. Any file can be attached locally, including audio and documents. `--dry-run` lists each file (kind, name, size, and path) and does not upload it. `~/…` is stored as the expanded path.
- **Site copy takes up to 4 images or videos.** The app, the local archive, and the network keep every file. Red Star accepts `image/*` and `video/*` only, at most 4, on the site copy. Audio, documents, and other types stay local: the picker shows them greyed out, with the reason, and they are never sent. More than 4 images or videos, with no choice, publish the first 4 of those and print a notice naming the ones left off. `--site-attachments` takes 1-based indexes (`1,3,5,6`). An index that names an ineligible file fails and names that file and the allowed kinds. The compose box opens a picker when there are more than 4 images or videos: Space toggles an eligible file, at most 4, Enter confirms, Esc leaves without publishing. Each stored file has `on_site` (older rows without the column count as on the site). Export JSON adds `"on_site"`. A confirmed publish still does not send.
- **TUI path fields expand `~` and Tab-complete.** Export › Path, Import › Path, Research › Export to, Research › Local file, and Start Scanning › Output file complete one real path segment on Tab, the same way the compose attach line does. The argv the child runs expands `~` and `~/…`, because that child is not a shell.

### Fixed

- **Terminal UI login.** Entering the emailed code no longer fails with "websocket closed" and then drops the attempt. The login keeps the LiveView socket alive with Phoenix heartbeats while the code is read, reconnects and retries verification if that socket closes, and stays on the code step when the code is wrong. Resend code (F2) asks for another code on the same login. A finished login is written to `~/.acp/session.json`, the same file the command line writes. The command line also heartbeats while it waits at the prompt.
- **Compressed export paths.** With Compress (tar.gz) on, a path that does not already end in `.tar.gz` or `.tgz` is no longer used as both the archive and the staging directory (that failed with "Is a directory"). A bare name gets `.tar.gz` appended (`resonance-cosmos` becomes `resonance-cosmos.tar.gz`). A directory, or a path ending in `/`, is `<dir>/<export-name>.tar.gz` (`Resonance Cosmos` becomes `resonance-cosmos.tar.gz`). The success log line is `wrote <final path>`. Export › Path still expands a leading `~` / `~/` before that child runs, so `~/resonance-cosmos.tar.gz` is `$HOME/resonance-cosmos.tar.gz` and not `./~/resonance-cosmos.tar.gz`.

### Not sent

- **The upload channel is not captured.** `allow_upload` and `lvu` are not built, so `media_attachments` and `file_attachments` are not encoded and no file bytes are sent. A confirmed publish still stops where 3.4.0 stopped: `validate` and `save` are not sent. Vote, subscribe, delete, edit, and repost are not in this release.

### Confirmed

- **The three attachment decisions.** `.jpeg` is accepted as jpg and `.tif` as tiff. There is no count cap and no size cap on the app, the archive, or the network. The site copy is a separate cap of 4. This release keeps 3.5.0. Interactions stay unstarted, and 3.10.0 and later, including the locked 3.22.0 re-key, are not shifted.
- **v3 historical posts.** [docs/history/v3/redstar-v3-prelaunch-post.md](docs/history/v3/redstar-v3-prelaunch-post.md) is the coming-soon draft. The README links it. [docs/history/v3/redstar-v3-launch-post.md](docs/history/v3/redstar-v3-launch-post.md) is the launch post, written as if 3.x has fully shipped, and it is published only when 3.x is done. At the 3.0 release, switch the README link to that launch post.
- **Server and relay packaging is parked on the alpha gate.** The server (`acp-rs`, `--serve`) and the relay (`acp-rs-relay`) each get a separate install, with its own binary, release artifact, and install path, and each stays available inside the app. No server or relay release is cut until writes and replies with attachments are functional and the decentralized network work is functional.
- **Not an alpha.** Alpha packaging waits until writes and replies with attachments are sent and the decentralized network work is completed. This version does not add an alpha tag or release assets.

## [3.4.0] - 2026-10-19

### Added

- **New post and Reply.** `n` opens a compose box. `r` on an open post replies; on a comment it pre-fills a `postRef`. `Q` quotes that post. `--new-post`, `--reply`, `--body`, `--in-reply-to`, `--quote`, and `--quote-post` do the same from the command line. The body is Markdown turned into the editor's ProseMirror document (paragraphs, lists, task lists, quotes, bold, italic, underline, links, mentions, and tags). Anything that schema cannot express is refused before a frame is built.
- **Safety.** Writes are on unless `ui.writes` is false or `ACP_WRITES=off` (the environment wins). The compose box starts on Cancel. `--yes` confirms; without it a terminal asks `Publish? [y/N]`. `--dry-run` prints the documented LiveView frames and opens no connection. A post or reply inside 30 seconds of the last confirmed attempt waits. A changed site script URL refuses writes until `--accept-site-update`.

### Fixed

- **Certificate lifetimes.** Includes the 3.3.2 correction: an authorizer certificate lasts 30 days, a node certificate lasts 365 days, and clock skew is 5 minutes. See [3.3.2](#332---2026-10-19).

### Not sent

- **The guided capture has not happened.** Confirming Publish does not open a connection and does not post. `--capture-reply` is hidden, copies nothing, and sends nothing. `_target`, hidden fields from a live page, the upload channel, `mention-query` / `tag-query` replies, and read-back of the saved post stay omitted. Attachments are refused and wait for 3.5.0. Vote, subscribe, delete, edit, and repost are later releases.

## [3.3.2] - 2026-10-19

### Fixed

- **Certificate lifetimes.** 3.3.0 and 3.3.1 signed a node certificate for 30 days and an authorizer certificate for 365 days. The decision from 2026-10-19 is the other way around: an authorizer certificate lasts 30 days, a node certificate lasts 365 days, and a verifier allows 5 minutes of clock skew. A new `--node-init` uses those values.

### Changed

- **Nodes created on 3.3.0 or 3.3.1.** Those certificates keep the reversed lifetimes. `--node-init` refuses to replace `~/.acp/node/keystore`, and there is no separate re-certify command. Remove `~/.acp/node` and run `--node-init` again. That creates a new node key, so the peer id changes, and signs it for 365 days. The account id does not change.

## [3.3.1] - 2026-10-19

### Fixed

- **Job elapsed time.** The job-details window measured every refresh from the start to the current time, so a finished, failed, or cancelled job kept counting. Elapsed now stops at the recorded end (finish, failure, or cancel). It still ticks while the job is running. A stopped job with no recorded end shows `unknown`. The header running mark and the status bar do not show a duration. Starting the terminal UI again does not rebuild jobs from log files.

### Documentation

- **Plan decisions.** [architecture/PLAN-3.0-CLIENT.md](architecture/PLAN-3.0-CLIENT.md) records the answers from review: an authorizer certificate lasts 30 days, a node certificate lasts 365 days, and clock skew is 5 minutes; renewal, the 7-day grace period, and revocation stay in their planned stages. A future daemon may unlock from the OS keyring (Secret Service, Keychain, Credential Manager) behind `SecretStore`. Public nodes are opt-in and land with `--serve` in the network-node stage. The first machine's history access is accepted (`allowed`, `max_nodes` 16). The 3.3.0 certificate constants are unchanged. The tombstone rule and the stage numbers after 3.21.0 were revised after this note. See Unreleased.

## [3.3.0] - 2026-10-19

### Added

- **Node identity.** `--node-init` generates an ed25519 signing key and an X25519 wrapping key on this machine. Those keys are not derived from the master. The master, rebuilt from the sealed phrase and the password (or from the phrase you type when there is no seal), signs an authorizer certificate and a node certificate, then it is dropped. The secret is sealed at `~/.acp/node/keystore` (mode `0600`) under the unlocked database key. A plain `identity.key` is not written. `--node-id` prints the name, the account id, the peer id, the fingerprint, six verification words, and whether the certificate still checks. The header shows `◉ name` when a node exists. Backups skip the keystore, so a restore does not clone the node. Certificate lifetimes in this release are the plan's proposed defaults (30 days for a node, 365 days for an authorizer, 5 minutes of clock skew). The format is in [docs/PROTOCOL.md](docs/PROTOCOL.md).

## [3.2.0] - 2026-10-19

### Added

- **Shared core crate.** `acp_red_star_node_core` holds the master secret, the 24-word phrase, the SLIP-39 shares, canonical CBOR, the HPKE wrap of an account key, and a `SecretStore` interface. The terminal app depends on it. A later relay-only package, tray app, or Dart FFI crate can depend on it without the terminal UI. Canonical CBOR is specified in [docs/PROTOCOL.md](docs/PROTOCOL.md), with test vectors. Signing and hashing use that encoding. There is no protobuf.

### Changed

- Nothing a current install reads or writes. Command-line flags, `preferences.toml`, the SQLCipher database, `rs.database.bootstrap`, `~/.acp/acp-id`, `~/.acp/recovery.seal`, and `.acpbackup` format version 1 are the same bytes as in 3.1.0.

## [3.1.0] - 2026-10-19

### Added

- **Job details.** Click the status bar, or the `⟳` running mark in the header, or press Tab until Status is focused and then Enter. The window shows the job name and what started it, whether it is running, finished, failed or cancelled, the start time and elapsed time, the current stage and a spinner, and a scrollable log tail. When several jobs are running they are a list. `c` or Cancel stops a job that is still running. Esc closes the window. The password and the database key are not shown. Encrypting the database, backup and restore still use the progress screen; they are not jobs in this window.

## [3.0.0] - 2026-10-18

### BREAKING CHANGES

- **A password is required to create or open the local database.** 2.x opened `~/.acp/ai/data/rs.database` with no password. 3.0.0 creates that file encrypted and asks for the password at startup, or reads `ACP_PASSWORD` when there is no terminal. The password is at least 8 characters and has to pass a strength check. There is no password reset.
- **An existing 2.x database is not opened.** Startup stops and tells you to run `acp-red-star --encrypt-db`. That command writes a `*.acpbackup` first, encrypts the database, checks row counts and a content hash, and deletes the plaintext file only after the encrypted copy matches.
- **Rollback.** Until `--encrypt-db` succeeds, a 2.x binary still reads the plaintext file. If the migration fails, that file is left in place. After it succeeds, 2.x cannot read the database, `rs.database.bootstrap`, or a `*.acpbackup`. Copy `rs.database` somewhere outside `~/.acp` before migrating if you may need to go back; that copy is what you put back to return to 2.x. 3.0.0 restores one of its own backups with `--restore`.
- **New files.** `rs.database.bootstrap` (next to the database) holds the wrapped local key. `~/.acp/acp-id` appears only after `--identity-init`. `~/.acp/recovery.seal` is the encrypted copy of the recovery phrase, written by the same command. `session.json` is unchanged and is still not inside a backup.

### Added

- **Master secret and `acp-id`.** `--identity-init` draws 256 bits, shows them as a 24-word BIP-39 phrase on a terminal, and derives the master from that seed and the password. You type the words again to confirm the paper copy. The words are not logged. An encrypted copy of the 256-bit entropy is stored at `~/.acp/recovery.seal` (mode `0600`), wrapped with Argon2id and XChaCha20-Poly1305, so `--show-phrase` can show them again after the password is entered. The password alone cannot rebuild the words. A phrase created before that file existed was shown once and cannot be shown again. The Argon2id salt of the master is `blake3("acp-master" ‖ seed)`, so the email is not an input. ed25519 and X25519 keys are HKDF labels of that master. `--acp-id` prints the pinned id and does not print the phrase. A wrong password still derives an id; the tool shows it and refuses unless it matches the pin.
- **After encryption.** Creating a database, and `--encrypt-db` after a 2.x migration, explain the recovery phrase on a terminal: what it is for, how to create it, what 3 of 5 shares means, how to store and check a paper copy, and how to restore a `.acpbackup`. The command offers to run `--identity-init` immediately. The terminal UI asks the same question and, if you skip it, opens Help with that text. `--help` and the Help screen (`?` / F1) include it. A non-interactive run does not print it.
- **`--show-phrase`.** Shows the 24 words again. It requires a terminal and always asks for the password, including when `ACP_PASSWORD` is set. It warns that the words will be on the screen and may stay in scrollback, lets you type a paper copy to check it, and offers to clear the terminal afterward. `--change-password` reseals the phrase with the new password when the phrase used the same password.
- **Recovery shares.** The 256-bit seed is split with SLIP-39, one group, 3 of 5, and an empty share passphrase. `--shares-print` writes five sheets. `--shares-verify` checks three to five shares and stores nothing secret. The password is not in the shares. Reissuing shares of the same seed does not invalidate older ones. The construction is checked against the official SLIP-39 test vectors.
- **Backup and restore.** `--backup` writes a `*.acpbackup` file: a consistent SQLite snapshot plus preferences, themes, research, attachments, the pinned `acp-id`, and `recovery.seal` when it exists. The archive is tar, then zstd, then XChaCha20-Poly1305 under a key from Argon2id. `--restore` checks the format, the password and, on a new machine, the phrase or three shares against the header `acp-id`. The seal inside the archive does not unlock a new computer; that still needs the phrase or three shares. `session.json` and the node secret are not in the archive. `--backup-schedule daily|weekly --keep N --dest DIR` is stored and the terminal UI reminds you when it is due. The headless job that runs the schedule is a later release.
- **Encryption at rest.** A database this program creates is SQLCipher. The on-disk key is a random 256-bit local key, wrapped under the password in `rs.database.bootstrap`. A separate account data key (epoch 1) is wrapped to that password and, when an `acp-id` is pinned, to the master with HPKE. Restore of a backup re-encrypts with a fresh local key.
- **Progress for the slow steps.** Creating a database, `--encrypt-db`, `--change-password`, `--backup`, and `--restore` run the long work off the thread that draws the terminal. While stdout is a terminal you get a spinner, the stage (deriving the key, encrypting the database, verifying, and the backup and restore stages), and the elapsed time. Keypresses are ignored until that step finishes, so it is not cancelled in the middle of a copy.
- **`--change-password`.** Rewraps the local database key and the account passphrase wrap. The account id does not change. A backup written with the old password still needs that password.

### Fixed

- **A running session does not ask for the password again.** Scrape, Research, Explore, Executive Board, Sync, and the other jobs are a second copy of this program. That copy did not have the key the terminal UI had just unlocked, so it exited with `Error: this database needs a password. Set ACP_PASSWORD, or run from a terminal.` The terminal UI copied that line into the status bar and opened the same text again over the post list, which is what was left on the screen when changing tabs. The unlocked key now stays in memory for the session and is passed to those jobs on a private pipe (not on the command line and not in the environment). It is wiped when the process exits. `ACP_PASSWORD` and the password prompt are only used at startup. A job that fails is reported in the status bar, and changing tabs redraws the whole screen.

### Changed

- **One Argon2id stretch per operation.** Migration used to stretch the password three times (the backup, the local key, and the account key) with no feedback. Those wraps now share one stretch and split the keys with HKDF. The parameters are unchanged: 64 MiB, three iterations, one lane. A password change stretches the old password and the new password once each when those wraps share a salt. An older wrap that stored its own salt still opens.
- **The SQLCipher password KDF is not stacked on Argon2id.** The local key is passed as `PRAGMA key = "x'<64 hex>'"`, which uses those bytes directly. SQLCipher's configured KDF stays PBKDF2-HMAC-SHA512 at 256,000 iterations and is what a passphrase key would run; a raw key does not run it. The page HMAC key is still derived with SQLCipher's 2-iteration step. That is the raw-key construction, and opening the file stays well under a millisecond. The temporary copy turns synchronous writes off and fsyncs once before it replaces the plaintext file.

## [2.24.1] - 2026-10-18

### Fixed

- **Bookmarks** push the hook `bookmark {id, state:"1"|"0"}` instead of the click `toggle-bookmark {id}`. `"1"` adds and `"0"` removes. That is the event the site's current page sends; the click name still worked at 2.5.0 and may disappear.

## [2.24.0] - 2026-10-18

### Added

- **Edit history** (`/rs/post/{thread}/{id}/edits`): `--edits THREAD/POST` (or a post id already stored). `p` › View edits opens the saved versions at once and refreshes them. Each version is a line diff against the previous one. Versions are stored in `post_versions` with `source = site`. The table also allows `peer`, `local` and `draft` for later local-first drafts; those are not written yet.

## [2.23.0] - 2026-10-18

### Added

- **Reposts list** (`/rs/post/{id}/reposts`): `--reposts ID`. `p` opens a post menu on the highlighted or open post; View reposts opens that list, saved locally and refreshed from the site.

## [2.22.0] - 2026-10-18

### Added

- **Thread sort:** `s` in an open thread cycles replies newest → oldest → top. Newest is the site default (no hook). Oldest and top send `sort {sort:"oldest"|"top"}` via `--sort-thread ID --thread-sort ORDER` and store `score`. A missing score does not blank one already stored. Top sort puts posts with no score last.

## [2.21.0] - 2026-10-18

### Added

- **Tag feeds** (`/rs/tag/{tag}`): `--tag TAG`. A `#tag` in a post body (and a Tags row on the detail) opens that feed, saved locally and refreshed from the site. `#post-…` anchors and `##` headings are not tags. Tags inside fenced or inline code are not clickable.
- **`body_json` and `post_tags`:** posts store the ProseMirror body as compact JSON when the site sends one, and tags from mention nodes plus `#word` in the text. A later save does not blank a JSON body or a tag list that this write doesn't carry.
- **`--backfill-tags`:** rewrites `post_tags` from what is already stored, then re-reads threads whose JSON body is still empty (one page per thread, with a pause). Optional `--nick` / `--nicks`. Stored post text is left as it is.

## [2.20.0] - 2026-10-18

### Added

- **Explore** (`/rs/explore` and `/rs/explore/latest`): `--explore` and `--explore-latest`.
  The **Explore** row under EB (and View › Explore) opens the saved order; View › Explore latest
  selects the Latest tab.
- **Site search** (`/rs/explore?q=`, form `search`, field `q`): `--site-search TEXT`. The Shift-S
  search modal has a **Where** choice, Local or Site. Site uses the keyword only and says so when
  nicks, dates, a post id or a kind are also filled in. An empty site keyword is an error. Local
  search is unchanged.

## [2.19.0] - 2026-10-18

### Added

- **Executive Board feed** (`/rs/eb`): `--eb` and `--eb-replies`. The **EB** row under Bookmarks
  in the nick bar (and View › Executive Board) opens the saved order at once and refreshes it
  from the site. View › EB includes replies sends the replies filter.

## [2.18.0] - 2026-10-18

### Added

- **Site-wide Top** (`/rs/top`): `--top [PERIOD]` with the five periods `day`, `week`, `month`,
  `year` and `all` (default `week`), and `--top-replies` for the feed's replies filter. The posts
  are stored and the order is kept in `site_feeds`, so View › Site Top (period and replies) opens
  the saved view at once and refreshes it from the site. The direct URL is read first; if it has
  no feed, the same `t` / `replies` fields are sent with `configure-feed`.

## [2.17.0] - 2026-10-18

### Added

- Post bodies render full Markdown (CommonMark plus GitHub extensions, parsed with
  `pulldown-cmark`): headings H1–H6 (H1 a banner with a `━` rule, H2 with a `─` rule), bold,
  italic, bold italic, ~~strikethrough~~, inline code, fenced and indented code blocks in a box
  with the language on the top edge, nested block quotes with a `┃` bar, bullet, ordered,
  nested and loose lists with hanging indents (`•` `◦` `▪` by depth, right-aligned numbers),
  task lists (`☐`/`☑`), tables with box-drawn borders, aligned columns and wrapped cells,
  horizontal rules, links and autolinks, images (alt text and URL), footnotes (`[1]` with the
  notes at the end), hard and soft line breaks, HTML entities, escaped characters and the
  common inline HTML tags (`<br>`, `<b>`, `<i>`, `<a href>` …).
- Links in post bodies are clickable in terminals with OSC 8 hyperlinks; the URL is shown
  after the link text when the text doesn't already contain it.
- Post bodies are tidied the way the site stores them: lines that start with a typed `•`
  (also tab-indented) become list items, and a lone indented line is text rather than code.
- Theme elements for Markdown: `h1` … `h6`, `quote`, `code_block`, `list_marker`, `task`,
  `table`, `table_header`, `rule` and `footnote`, with defaults in all seven shipped themes.
  The theme editor's sample post shows them, so they can be highlighted.

### Fixed

- Post bodies: list items separated by blank lines (loose lists) lost their bullet or number,
  because the item's paragraph replaced the prefix the item had just set. Every list item now
  shows its `•` or number, also when it starts with bold text, code or a link, when it opens
  with a sublist or is empty, with wrapped lines indented under the item text.

## [2.16.0] - 2026-10-18

### Added

- Theme editor: the preview adds a *Sample dialog* frame with the parts the post view doesn't
  show (dialog, focused and plain labels, a text field, the Post/Reply tags, a link, a deleted
  post, inline code, dimmed text, a button, a selected row), so they can be seen and highlighted.
- Theme editor: `r` reverts only the selected palette colour, element or element key to the saved
  file (`Ctrl-R` still reverts everything).
- Theme editor: saving asks first when the file changed on disk since the editor read it
  (Overwrite / Load it and drop your changes / Cancel), and before overwriting a broken file
  (Overwrite / Cancel). Both also apply to *Save* in the unsaved-changes question.
- Theme editor: when the theme uses `#rrggbb` colours and `$COLORTERM` isn't `truecolor`/`24bit`,
  the preview notes that the terminal may show the nearest of 256 colours.
- A test that the shipped `themes/default.toml` matches its expected content exactly.

### Changed

- The header theme switch reads `‹name: missing› ▾` or `‹name: broken› ▾` instead of `‹name ⚠› ▾`
  when the theme can't be used.
- The warnings of one theme file (unknown or bad keys, also those from the `[theme]` migration)
  are one notice (`warning: …/x.toml: 3 keys skipped: a; b; c`) instead of one line each.

## [2.15.0] - 2026-10-17

### Added
- **Theme editor: the selected colour or element is highlighted in the live preview.** The
  preview is drawn a second time with that row's colours replaced by unused sentinel colours, and
  the cells that differ are shown in reverse video. `h` toggles it; the preview title says
  `‹name› highlighted (h)`, or `‹name› isn't on screen` when that element isn't in the preview.
- **Starter themes:** `--theme-starter NAME` and *Theme: starter [name]* in the command palette
  (`theme starter mine`; no name: the first free `starter`, `starter-2`, …) write a minimal
  generated theme with plain terminal colours. No download; atomic, mode 0600, never over an
  existing file (the CLI fails if it exists).
- **The `F5` list with no themes** offers *Download the default theme* and *Create starter
  theme* (which also selects it).
- **The header shows `‹default · downloading…› ▾`** while the first-run download runs.

### Changed
- When a theme download fails (`--theme-init`, *Theme: init*, the first run), the notice also
  suggests the starter theme.
- In the theme editor `h` now toggles the preview highlight; it no longer hides an element's keys
  (`←` still does).

## [2.14.0] - 2026-10-17

### Added
- **First-run theme download.** When `~/.acp/themes` is empty, the TUI starts at once with the
  terminal's colours and downloads `default.toml` in the background from
  `https://raw.githubusercontent.com/m3talsmith/acp-red-star/v<version>/themes/default.toml`
  with the program's HTTP client and no cookies (3 s timeout, 64 KiB cap). The file must parse as
  a theme, is written atomically with mode 0600 without replacing an existing file, and is applied
  live once it arrives unless you picked another theme meanwhile. Offline or on any failure the
  terminal's colours stay, with a notice explaining the fix. `--list-themes` and `--theme` do the
  same download on a first run.
- `--theme-init [NAME|all]` and *Theme: init* in the command palette download a theme (the
  default, any theme in the repo's `themes/`, or all of them) on request.
- Opt-out: `ACP_THEME_DOWNLOAD=off` or `[ui] theme_download = false`. `ACP_THEME_URL` points
  the download somewhere else (used by the tests' mock server).

### Changed
- Themes are read only from `~/.acp/themes/<name>.toml` (under `$HOME`).
- Missing- and broken-theme notices now point to `--theme-init` instead of reinstalling.

### Removed
- The system theme folders (`/usr/share/acp-red-star/themes`, `themes/` and
  `../share/acp-red-star/themes` next to the binary, the source tree's `themes/`) and the
  first-run copy from them.
- Packages (debian, Fedora, Arch, `scripts/stage.sh`) no longer install theme files. The repo
  still has `themes/*.toml`; that is where the download comes from.

## [2.13.0] - 2026-10-17

### Added
- Six communist-themed themes, each with dark and light variants and at least 3:1 text contrast,
  shipped in `themes/` (and the packages) so the first run installs them:
  `soviet` (Soviet red and gold), `constructivist` (red, black and cream after Rodchenko and
  Lissitzky), `cuba` (olive green, red and black), `red-star` (deep crimson with ACP red),
  `chinese-poster` (vermilion and golden yellow) and `plattenbau` (East German concrete greys with
  a red accent). Pick one with F5 or `--theme NAME`. An existing
  `~/.acp/themes` is left alone; copy the new files from `/usr/share/acp-red-star/themes/` (or
  the source tree's `themes/`) to get them there.
- A test that every shipped theme loads without warnings and stays above 3:1 contrast in both variants.

## [2.12.0] - 2026-10-17

### Added
- **Theme files.** Every TUI theme, the default included, is a TOML file in `~/.acp/themes/<name>.toml`
  (names: `a-z`, `0-9`, `-`, `_`, at most 32). Nothing is built into the binary: the default ships
  as `themes/default.toml` (packages: `/usr/share/acp-red-star/themes/`) and the first run copies the
  shipped themes into an empty `~/.acp/themes` (looking in `/usr/share/acp-red-star/themes`, then
  `themes/` next to the executable, then the source tree). It reproduces the old look exactly
  (`#dc2626` accent). Format v1: `version`, `name`, `description`, then `[dark]` and `[light]`, each
  with a `palette` (colours as `0`–`255`, `#rrggbb`/`#rgb`, the 16 ANSI names or `reset`; `"@name"`
  references another entry) and optional per-element `styles` (`fg`, `bg`, `border`, `bold`,
  `italic`, `underline`, `reverse`, `dim`) for 27 elements. Borders and padding stay in code.
- `[ui] theme` and `[ui] theme_mode = "dark" | "light" | "system"` in `preferences.toml`; `system`
  reads `COLORFGBG`, else asks the terminal for its background (OSC 11), else dark.
- **Switching:** a `‹name› ▾` switch in the header and **F5** open the theme picker (moving previews,
  Enter uses and remembers, Esc keeps the old one), which ends in "Edit themes…". Command palette:
  Theme: use / list / edit / reload / export. CLI: `--theme NAME` (this run) and `--list-themes`.
- **Theme editor (Shift-F5):** full screen; the palette and every element on the left with swatches,
  `*` for unsaved changes and `⚠` below 3:1 contrast; a live preview on the right drawn by the real UI
  code over sample posts (never your database). Tab flips dark / light; colours by typing (hex,
  0-255, names, `@refs`) or from a grid (16 colours, the 216 cube, 24 greys, palette references).
  `^S` save, `^R` revert, `^N` new, `^D` duplicate, `^X` delete (asks first), `[` `]` other themes,
  Esc asks Save / Discard / Cancel when there are unsaved changes.
- Theme files are written atomically with mode 0600; Theme: export writes a copy to
  `~/.acp/ai/data/exports/themes/`.
- **Safety:** A missing, broken (not TOML), too big (over 64 KiB) or newer-format theme falls back to the
  terminal's own colours (`Color::Reset`), so the layout still works, with a notice naming the file
  and how to fix it; such files are never overwritten automatically. Unknown keys give one warning
  each; missing keys fall back. Names that could leave the themes folder are refused.

### Changed
- **Start Scanning moved from F5 to F6**; F5 is the theme picker.
- The old `[theme]` colours in `preferences.toml` are converted once into `~/.acp/themes/custom.toml`
  (selected, with a notice); the old keys stay in the file but are ignored.

## [2.11.1] - 2026-10-17

### Changed
- The TUI header is one row taller (4 rows): the big "ACP" sits on the lower three rows so it
  isn't pressed against the top edge, and the name, version, session and running job sit on its
  middle row. The TUI's minimum size is now 60×19.

### Fixed
- The red-star logo in the header now looks like `assets/red-star.png` (a ring with a star in
  it): it fills all four header rows, square on screen, and is drawn in white straight on the red
  bar instead of in a small light badge. Without terminal graphics (half blocks: the Linux console,
  tmux, SSH, waypipe) it is a 16×12-dot sextant silhouette made by box-filtering the PNG's alpha,
  computed once per size; with kitty, sixel or iTerm2 the PNG is composited white-on-red first.

## [2.11.0] - 2026-10-17

### Added
- **Empty-Body animation** in the TUI: while the Body is empty a text animation loops under the
  hint, shrunk to fit and centred in the muted colour (`[ui] empty_animation_color = "accent"` for
  the accent). It pauses when the Body is smaller than 24×8 and redraws only when the frame
  changes. The built-in clip is the first 10 seconds of "Debating Is Man the Measure of All
  Things?" (IGG | Qizilbash) as 120×34 text art at 12 fps, used with permission of the ACP
  (`assets/empty-body.NOTICE.md`, 35 KB, embedded in the binary); a procedural turning red star
  with an ACP wordmark is the fallback. `[ui] empty_animation = false|"off"|"star"`,
  `[ui] empty_animation_path`, and `ACP_EMPTY_ANIMATION=off|star|on`.
- `tools/acp-anim`: turns the first N seconds of a local video or a URL (via yt-dlp) into a frames
  file, by default `~/.acp/animations/empty-body.acpanim.gz`, which the TUI plays instead of the
  built-in clip. Packages install it as `/usr/share/acp-red-star/tools/acp-anim`.

## [2.10.0] - 2026-10-17

### Changed
- All TUI styling now goes through [ratatui-lipgloss](https://crates.io/crates/ratatui-lipgloss)
  1.1 (a Lip Gloss port for ratatui): `tui::theme` builds every text style and every box (border
  kind, border colour, padding) as a lipgloss `Style`, and widgets only pick one. The look is
  unchanged (checked cell for cell against 2.9.0), the accent is still `#dc2626`, and `[theme]`
  overrides work as before.

## [2.9.0] - 2026-10-17

### Added
- **Research projects**: named collections of posts, replies, search results, local files and web
  pages, each with an optional note, stored in the new `research_projects` / `research_items`
  tables. Flags: `--research-list`, `--research-create NAME [--research-description TEXT]`,
  `--research-add NAME` with `--post-ids IDS`, `--search TEXT [--nicks]`, `--file PATH [--no-copy]`
  or `--url URL` (plus `--note`), `--research-show NAME` and `--research-export NAME --export FILE`.
  Local files keep their metadata (and text) and are copied to `~/.acp/research/<project>/files/`;
  web pages are fetched once without the acp.us cookies and keep their title, readable text and an
  HTML snapshot. Exports add a `research` section to the JSON and HTML; bundles carry the files.
- TUI: a `◈` button on every card and detail (or `r`) opens a research picker — add this post or
  reply, the posts it links, its attachment files, the web pages in its body, or the whole view /
  search results to an existing or new project, with a note. A Research menu (`Alt-r`): Projects…
  (pick or create), Show project (its items as cards), add a local file or a web page URL, Google
  Drive and Telegram (pinned for later), and Export project.
### Changed
- The menu bar's scan button shortens to `▶ Scan` when the window is too narrow for every command.
### Fixed
- Readable web text no longer puts a space before punctuation after inline tags.

## [2.8.0] - 2026-10-17

### Added
- **Export basket** in the TUI: a `+` button next to the star on every card and detail (or `x`)
  adds that post or reply (`✓` when in; click again to remove). Export › Show basket, Export basket
  (uses Export's Path / Format / Name / Compress) and Clear basket; Export › From has a Basket
  choice. The basket lasts for the session.
- `--post-ids IDS`: offline export of exactly these posts and replies (in the order given) with
  their attachments, to `--export FILE` or stdout / `--output`; default name `basket-<ts>`. Unknown
  ids are warned about; none found is an error.

## [2.7.0] - 2026-10-17

### Added
- **Pictures in the TUI**: stored post images show inline in a detail (a 14-row
  slot under the body, drawn once fully on screen), decoded on a background thread and shrunk to
  1024 px. The graphics protocol is detected by querying the terminal (kitty, sixel, iTerm2) with a
  Unicode half-block fallback (tmux always falls back); `ACP_IMAGE_PROTOCOL` forces one,
  `ACP_IMAGES=off` disables pictures. The encoders (kitty, sixel, iTerm2, half blocks) are built
  in: ratatui-image was not used because its sixel quantiser needs Rust 1.90 (MSRV is 1.88).
  AVIF/HEIC are converted once to PNG in `~/.acp/cache/thumbs/` with `avifdec`, `magick` or `ffmpeg`.
- Packaging recommends an AVIF converter (libavif / ImageMagick / ffmpeg) and xdg-utils.

### Changed
- The TUI header is three rows tall: the red star logo from `assets/red-star.png` (on a light badge
  so it shows on the red bar), a big "ACP" drawn with tui-big-text (third-height pixels), and the
  name, version, session and running job vertically centred. Minimum terminal size is now 60×18.

## [2.6.0] - 2026-10-17

### Added
- TUI post bodies render as **Markdown** (pulldown-cmark → styled, word-wrapped lines): headings,
  bold / italic / strikethrough, inline and block code, bullet and numbered lists with hanging
  indents, block quotes, links (the URL is shown when it differs from the text). Line breaks and
  paragraph breaks are kept.
- **Attachments are links** in a post detail: click, or `←`/`→` then `Enter`, opens the stored file
  with `xdg-open` (`open` on macOS; `ACP_OPENER` overrides), detached from the TUI. Missing files say
  so. Where supported they are also OSC 8 `file://` hyperlinks (off for `TERM=linux`/`dumb` or with
  `ACP_NO_OSC8`).

### Changed
- One column of padding between the borders of cards and details and their text.

## [2.5.0] - 2026-10-17

### Added
- TUI **nick filter**: a field at the top of the nick bar (click it, or `/` from the nick bar or
  Body) narrows the list as you type. Case-insensitive; substring matches on the nick or display
  name come first, then fuzzy in-order matches (`hzd` → `hazaldin`). Shows `matched/total` in the
  field and the bar title; `Esc` or the `x` clears it. Selection, Ctrl/Shift-click, `Shift-↑/↓`
  and `Ctrl-A` act on the shown nicks; selected nicks that are filtered out stay selected. The
  ★ Bookmarks row stays visible, and the filter survives view-mode switches, searches and
  refreshes.

### Changed
- `Ctrl-A` in the nick bar adds the shown nicks to the selection (with no filter: every nick, as
  before).

### Notes
- Bookmark sync is confirmed against the live site: the `/rs/bookmarks` page and the
  `toggle-bookmark` LiveView event are no longer marked as guesses.

## [2.4.0] - 2026-10-16

### Added
- TUI **previous / next** through the detail stack: `◂ prev` and `next ▸` on the Body's navigation
  bar with an `n/m` position, `,` / `.`, `Alt-←` / `Alt-→`, the palette, and View › Previous post /
  Next post. Prev steps back down to the list; next reopens what you closed (also after `Esc`, a
  header click or the Grid's Close). Opening a different post or link starts a new trail.

### Changed
- The flat view modes (Stack, Pane, carousels, Grid) allow up to 256 open posts; Nested keeps its
  limit of 8.

## [2.3.0] - 2026-10-16

### Added
- TUI **view modes**, switched with the `[View: …]` button on the Body's navigation bar, `v` / `V`,
  or the new View menu:
  - **Nested** (the 2.0 layout, still the default).
  - **Stack**: one endless column of full-height posts; details open flat, without nesting.
  - **Stack + pane**: the stack above a content pane showing the clicked post; `↑`/`↓` move the
    list and the pane follows.
  - **Carousel ∞** (wraps around) and **Carousel** (stops at the ends): one post at a time with
    its neighbours peeking at the sides; `←`/`→` or a click on a side flips.
  - **Grid**: cards in as many columns as fit; arrows move by cell and row; a post opens in a
    modal with a **Close** button.
- The read-only real-database smoke render now draws every view mode and the bookmarks view.

## [2.2.0] - 2026-10-16

### Added
- **Sync.** `--sync` logs in, syncs bookmarks both ways (skip with `--no-bookmarks`) and
  incrementally scrapes `--nick`/`--nicks` if given; it writes no export unless `--output` is set
  and never prompts, so it suits cron.
- TUI **Sync menu**: Auto sync on a timer (Interval, default 15 minutes), Nicks (blank: the
  selection), Mentions (off), Bookmarks (on), Sync now, and Save as default (`[sync]` in
  `~/.acp/preferences.toml`, loaded at start). Automatic syncs run as background jobs only when
  nothing else runs, check the session first, and skip quietly when offline or logged out; their
  failures go to the status line instead of a modal.

## [2.1.0] - 2026-10-16

### Added
- **Bookmarks**, local first and synced with the site:
  - `--bookmark IDS` / `--unbookmark IDS` / `--list-bookmarks` (offline) keep them in a new
    `bookmarks` table (created on first use; removed by `--clear-all`).
  - `--sync-bookmarks` logs in, reads the site's bookmarks (`/rs/bookmarks`, storing their posts),
    and pushes local additions and removals (`toggle-bookmark`). Local changes win; bookmarks
    removed on the site are dropped. The page path and event are inferred (override the path with
    `ACP_BOOKMARKS_PATH`).
  - TUI: a clickable `☆`/`★` star on every card and detail (or `b`), stored by a background side
    job that runs offline and next to a scan; a **★ Bookmarks (n)** row on the nick bar (or `B`)
    shows only bookmarked posts and comments; a Bookmarks menu (Show / Bookmark current post /
    Sync with acp.us).

### Changed
- The menu bar drops its separators (then its padding) when the terminal is too narrow.
- Database writers wait up to 10 s for a lock instead of failing, so TUI bookmark writes and a
  running scan don't collide.

## [2.0.0] - 2026-10-16

Major release: a bare `acp-red-star` now opens a terminal UI instead of prompting for nicks and
scraping. Every flag still works headless; the old bare behaviour is `--scrape`.

### Added
- **Terminal UI** (ratatui + crossterm, `src/tui/`), opened with no flags or `--tui`:
  - top bar, menu bar, nick bar with post counts, Body, status/log line; mouse and keyboard for
    every action; command palette (`Ctrl-Space` / `Ctrl-P`); Help modal generated from `--help`
    plus the key bindings.
  - offline, read-only browsing of the database; **multi-select nicks** (`Space`, `Shift-↑/↓`,
    `Ctrl-A`, Ctrl/Shift-click).
  - post details at parity with the HTML export; the list slides up when a detail opens, linked
    posts (reply-to, thread, ids in the body, replies) nest inside and push the previous detail up
    (animated; at most 8 deep; ids not in the database are flagged).
  - search replaces the Body in place with a `<` back button; `Shift-S` modal filters by keyword,
    nicks, post id (post + thread), date range and posts/replies; `Ctrl-S` clears.
  - menu bar built from the CLI flags (Scrape, Search, Export, Import, List, Maintain, Session,
    Debug, Help, Quit) with checkbox / dropdown / text / button options and `*` for required ones.
  - **Start Scanning** (and Export / Import / Maintain) build the CLI argv, validate it with clap and
    run the headless binary as a child process in its own process group, logging to
    `~/.acp/logs/tui-<kind>-<time>.log` (0600). **Cancel** kills it; **quitting during a scan
    detaches it** to keep running in the background (or cancels it, if you choose).
  - **bulk export from the selection** (selected nicks, current search, or everything).
  - Start Scanning while logged out opens the **Login modal** (email, then the code or login link)
    and starts the scan once logged in; Session › Log out deletes the saved session and returns to
    the Login modal. Sessions persist in `~/.acp/session.json` (0600) as since 1.21.
  - confirm modals for Remove nicks and Clear all (type `yes`).
  - **theme**: ACP red `#dc2626` accent by default; `[theme]` in `~/.acp/preferences.toml`.
- `--tui` (force the TUI; `--nick(s)` pre-select, logging flags pre-tick Debug) and `--scrape`
  (headless scrape that asks for nicks).
- `login_flow` (caller-supplied email/code), `session_valid`, `set_log_sink`, `list_nicks_conn`.
- Tests: TUI unit tests for the detail stack, search back, Args-from-menu mapping, layout
  geometry, modals, the read-only store, jobs and headless renders; Gherkin `tui.feature` for the
  entry modes and the argvs the Scrape / Export / Maintain menus build, against the mock site.

### Changed
- **Breaking:** no flags (or only logging flags) opens the TUI. Scripts that relied on the bare
  prompt-and-scrape should pass `--scrape` (or `--nick`/`--nicks`).
- Saving the login email keeps other settings in `preferences.toml`.

## [1.26.1] - 2026-10-16

### Fixed
- **Duplicate attachments from re-signed media links.** The site serves media through presigned URLs
  whose `X-Amz-Date`/`X-Amz-Signature` change on every page load. Files were named by a hash of the
  whole URL, so every scrape downloaded each image again under a new name and added another
  `attachments` row (hazaldin's 17 images had become 68 rows and files). Files are now keyed by the URL
  without its query string, a fresh link replaces the stored row for the same file, and DB exports
  (`--export`) list each file once even for rows stored before this fix. Scrape exports were already
  correct; DB exports and the attachments directory were the bloated ones.
- Tests: unit test for URL keys; e2e scenario re-scraping with re-signed links.

## [1.26.0] - 2026-10-16

One feature/fix since 1.25.1 (minor 25 → 26).

### Fixed
1. **Truncated post and comment text.** Feeds send long posts and replies shortened
   (`body_truncated: true`, a cut `body` doc and `body_text_summary`: the site's "show more"), and the
   scraper stored that preview. Every truncated feed item is now completed from its thread page
   (`/rs/post/{thread}`), which has the full body of the root and every reply; one fetch per thread,
   shared with the reply crawl, with a 250 ms pause after each fill-in fetch.
2. **Paragraph breaks.** Bodies keep a blank line between paragraphs (ProseMirror blocks and HTML
   `<p>`/`<div>`; `<br>`/hard breaks stay single newlines). HTML exports render each paragraph as `<p>`.

### Added
- `--refetch-truncated`: backfill stored rows whose body isn't known to be complete, for the given nicks
  or (with no nick) every nick and nothing else. Prints rows checked / completed / already full / not found.
- `posts.body_state` column (`full` / `truncated` / `unknown`), added in place to older databases
  (existing rows become `unknown`). A row stored `truncated` is re-read on the next scrape that sees it,
  and saving a full body over a non-`full` row replaces its text.
- JSON exports carry `"truncated"` per post; scrape summaries report truncated feed bodies and how many
  were completed.
- Tests: unit tests for paragraphs, truncation flags, the migration and backfill selection; e2e
  `truncated.feature` (mock feed with truncated post + reply, backfill, re-scrape repair).

## [1.25.1] - 2026-10-16

Bug fix since 1.25.0 (patch).

### Fixed
1. **`--said-by` exports looked like "comments only"**: the nick's own top-level posts were exported, but
   interleaved by date with their replies, with nothing marking which was which; replies in other people's
   threads (newest) filled the top of the page. Exports now separate and label them:
   - JSON: every node has `"kind": "post"` or `"reply"`; the document has `"counts": {"posts", "replies"}`;
     top-level `posts` lists thread posts first, then replies whose parent isn't in the export.
   - HTML: a "N posts · M replies" line, a **Posts** section, a **Replies in other threads** section, and a
     Post/Reply badge (`kind-post`/`kind-reply` class) on every article.
   Applies to every export (not only `--said-by`); `format_version` stays 2 (fields are additive, import ignores them).
2. `--said-by` help/docs now say explicitly that it keeps the nick's top-level posts **and** their replies.
3. Tests: unit tests for JSON kinds/counts/order and HTML sections/badges; e2e `--said-by --format json,html` scenario.

## [1.25.0] - 2026-10-15

One feature since 1.24.0 (minor 24 → 25).

### Added
1. **Several export formats per run**: `--format` takes a comma-separated list or can be repeated
   (`--format json,html`, `--format json --format html`); duplicates are dropped. With `--output <file>`
   / `--export <file>` each format is written as `<stem>.<ext>`; with a directory as `<dir>/<name>.<ext>`;
   with `--compress --tar-gz` all formats share one archive folder and `attachments/`. Several formats to
   stdout are rejected with a clear error. The 0-post empty-export skip applies to every format.
   `--list-nicks --format json` still selects JSON when `json` is among the formats.
2. Tests: CLI parsing of comma/repeated formats, output-path rule, JSON+HTML written side by side, both
   formats in one archive, empty-run skip for both, stdout rejection; two new e2e scenarios.

### Documentation
1. README shows the current version under the banner and a refreshed `--help` block; examples use the real
   `@hazaldin` profile; USAGE/README describe unknown-nick errors, empty-export skipping and `--debug`
   redaction; packaging metadata (PKGBUILD, RPM spec, debian/changelog, INSTALL.md) synced to 1.24.0.
2. New unit test: an empty run leaves an existing export file untouched.

## [1.24.0] - 2026-10-15

One security hardening since 1.23.0 (minor 23 → 24).

### Security
1. **Debug logs no longer leak session secrets**: `--debug` / `ACP_DEBUG=1` wire frames
   (`dbg_log`) now run through `redact_secrets` before stderr or `success.log`. JSON values
   for `_csrf_token`, `session`, `static`, cookies, and bearer-style tokens are replaced with
   `[REDACTED]`, as are `_csrf_token=` query params, Phoenix signed blobs (`SFMyNTY.…`), and
   long hidden-input `value="…"` attributes that carry CSRF tokens in LiveView HTML diffs.
   Event names, topics, and feed structure stay visible.

## [1.23.0] - 2026-10-15

Two fixes since 1.22.0, released together (minor 22 → 23).

### Fixed
1. **Unknown nick no longer scrapes Trending**: ACP answers `/rs/@{nick}` for a nick that does not
   exist with a 302 to `/rs` and the flash "No profile exists with the username @{nick}". The HTTP
   client follows redirects, so the Trending feed was scraped as if it were the profile (and
   `--said-by` then kept nothing). `fetch_feed` now checks that a profile URL landed on that same
   profile (and that the page has no "No profile exists" flash) and fails the nick with a clear
   error, which also goes to `error.log` when logging is on.
2. **No empty exports**: when a run kept 0 posts (and 0 mention hits), no export file is written,
   so an earlier good export of the same name is no longer overwritten with an empty one. A warning
   explains why nothing was written.

## [1.22.0] - 2026-10-15

One new feature since 1.21.0 (minor 21 → 22).

### Added
1. **File logging**: `--enable-logging` appends every message from the logging macros, one
   timestamped line each, to `~/.acp/logs/success.log` (progress, verbose detail, debug wire
   frames) and `~/.acp/logs/error.log` (errors and warnings). `--log <PATH>` and
   `--error-log <PATH>` override either file and turn logging on by themselves. `--debug`
   (or `ACP_DEBUG=1`) turns logging on too. The log directory is created as needed and files are
   opened in append mode. Stderr still follows `-q` / `-v` / `--debug` as before; files always get
   the verbose record. Paths live in `config.rs` (`LOGS_REL_DIR`, `SUCCESS_LOG_NAME`, `ERROR_LOG_NAME`).

## [1.21.0] - 2026-10-15

One new feature since 1.20.0 (minor 20 → 21).

### Added
1. **Remembered login**: after a successful login the session cookies are saved to
   `~/.acp/session.json` (owner-only, mode `0600`). The next run loads them, and if `/rs` no
   longer redirects to `/auth` it prints "Already logged in." and skips the one-time code.
   **`--logout`** (offline) clears the session and deletes the file; the next scrape asks for a
   code again. Cookie values are never logged. `logout`, `save_session` and `load_session` are
   exported from `auth::session` for the coming TUI.

## [1.20.0] - 2026-10-15

Three changes since 1.17.0 (minor 17 → 20).

### Added
1. **Immediate-parent relationship in the database**: new `posts.in_reply_to_id` column holds
   the post or comment a reply answers. The site keeps threads flat (`thread_id` only) and marks
   a reply to a comment with a `postRef` node in its body; that id is stored. `parent_post_id`
   keeps meaning the thread root. Both columns are indexed. Existing databases upgrade in place
   on first use (column added, replies backfilled with their thread root); a later scrape that
   sees the reply again records the real comment.
2. **Parent links in HTML exports**: each post is an `<article id="post-{id}">`, and every reply
   shows "Replying to {id}" (plus "thread {id}" when it answers a comment). Links jump to the
   anchor when the parent is in the export, otherwise to the post on acp.us.
3. **`in_reply_to_id` in JSON exports and imports**, next to `parent_post_id`. Imports of older
   exports fall back to the thread root. Searches now include each match's immediate parent too.

### Changed
- HTML anchors are now `#post-{id}` (were `#p-{id}`); the Mentions list links to them.

## [1.17.0] - 2026-10-15

One change since 1.16.0 (minor 16 → 17).

### Added
1. **`--said-by`**: limit a scrape to what the nick said — their own posts and their own replies
   from the profile's Posts and "Posts and replies" feeds. Other members' replies in the nick's
   threads are not crawled and the mentions crawl is skipped; a thread page is read only to
   recover a blank body of the nick's own item. Default scraping is unchanged.

## [1.16.0] - 2026-10-15

One change since 1.15.0 (minor 15 → 16).

### Fixed
1. **Profile photos for reply-only members** (e.g. `@haz`): the photo was only read from a
   top-level `author` on feed items, so replies that wrap the post (`{"reply": {...}}`,
   `{"post": {...}}`) or nest the author elsewhere left the profile with no photo, falling back to
   the site's `banner.jpg`. Feed items are now searched at any depth for the member's own author
   object (matched by `username`), plus flat `author_*` fields; site share images are skipped
   wherever they appear, so a real `media.acp.us` photo later in the data still wins. With
   `--debug`, a profile with no photo logs the first feed item so its shape can be checked.

## [1.15.0] - 2026-10-15

Three changes since 1.12.0 (minor 12 → 15).

### Fixed
1. **Blank posts**: replies (and some roots) were stored with empty `title`/`content`. Post bodies
   were only read as a ProseMirror doc in `body` (object or JSON string); a `body` that was HTML or
   plain text, or missing from the feed item, was silently dropped. Bodies are now read from
   `body` in any shape (ProseMirror, HTML, plain text, `{html|text|markdown}`), then `body_html`,
   `body_text`, `body_text_summary`, `text`, `content`, and wrapped `post`/`reply` objects. A post
   that still has no text and no media is read from its thread page. Blank rows already in the
   database are no longer treated as "known", so the next scrape re-fetches them and fills in
   their title/content in place (no duplicates, nothing deleted).
2. **Profile images**: the profile's photo is downloaded to
   `~/.acp/ai/data/attachments/profiles/{nick}.{ext}`, recorded as `avatar_path`, copied into
   exports as `attachments/profiles/…`, and shown in the HTML export's profile header. Author data
   is also read from the "Posts and replies" feed, so reply-only nicks get theirs. The site's
   generic `og:image` banner and `og:title` (e.g. "Trending") are no longer mistaken for a
   profile photo or display name.

### Changed
3. **Compressed exports clean up after themselves**: after the `.tar.gz` is written, the
   uncompressed `<name>/` folder beside it is deleted. If compression fails, the folder is kept.
- Project URLs (Cargo `repository`/`homepage`, Arch `url`, Fedora `URL`, Debian `Homepage`/`Source`,
  and the clone commands in README, INSTALL and USAGE) now point at the public GitHub mirror,
  https://github.com/m3talsmith/acp-rs-scraper, so anyone can clone and package it.

## [1.12.0] - 2026-10-15

Eight changes since 1.4.0 (minor 4 → 12): a cleanup release, plus the real "Posts and replies" mechanism and one bug fix.

### Changed
1. **Domain modules**: the crate is now a library (`src/lib.rs`) with a thin binary, split into
   `cli`, `config`, `logging`, `http`, `auth`, `liveview`, `scrape/{feed,thread,profile,mentions}`,
   `media`, `storage/{db,schema}`, `export/{json,html,archive}`, `import`, `maintenance`, `app`.
2. **Constants in one place**: `src/config.rs` (paths, site URLs, LiveView events, selectors, export
   format) grouped by domain; SQL schemas in `src/storage/schema.rs`. The site origin is configurable
   with `ACP_BASE_URL` (default `https://acp.us`).
3. **Tests moved out of source files** into per-module `tests.rs` files.
4. **Doc comments on every module, type, field and function** (`#![warn(missing_docs)]`; rustdoc clean).
5. **"Posts and replies" uses the verified site mechanism**: the profile Filter dialog
   (`configure-feed`, `replies=1`) → `/rs/@nick?replies=1`, loaded directly and paged with `page-next`,
   falling back to submitting the form. Replaces the 1.4.0 runtime tab heuristic. Still merged by
   `post_id`; `/rs/@nick/top?t=all` documented as the profile Top feed.

### Added
6. **Gherkin end-to-end suite** (`cucumber`): `tests/features/*.feature` + `tests/e2e.rs`, running the
   real binary against a mock acp.us (HTTP + LiveView websocket, OTP sign-in) with a temporary HOME:
   login, paging, threads/replies, attachments, incremental/`--full`, mentions, the replies filter and
   its fallback, search, export/import round trip, `--remove-nick`, `--clear-all`. Never touches the real site.

### Fixed
- Pre-commit credential hook matches token formats (AWS `AKIA…`, Google `AIza…`, GitHub, Slack, …)
  case-sensitively, so generated rustdoc search indexes no longer trip false positives; the
  password/secret assignment check still ignores case.
7. **First scrape on a fresh HOME**: saving the profile before any post created the data directory
   failed with "unable to open database file"; every write now creates `~/.acp/ai/data` first
   (found by the new end-to-end suite).

### Documentation
8. USAGE: "Posts and replies" filter, code layout and tests, `ACP_BASE_URL`; API docs regenerated.
   README opens with a Red Star banner, then an **Install** section (clone + `scripts/install.sh`)
   followed by the full `--help` output, so new users can install and start scraping without
   leaving the front page.

## [1.4.0] - 2026-10-15

### Added
- **"Posts & replies" profile tab**: nick scrapes also walk the profile's Posts & replies tab
  (the nick's replies in other threads), paged to the end and merged by `post_id` (never stored
  twice). Replies keep their thread root as `parent_post_id`; incremental/`--full` rules match
  the Posts feed. The tab is discovered at runtime from the logged-in profile HTML (link,
  `phx-click` event + `phx-value-*`, or JS push/patch); mocked tests only, not yet verified live.
- **Maintenance** (offline, under a new "Maintenance" heading in `--help`)
  - `--remove-nick @a,@b`: deletes the nicks' posts, other people's replies in their
    threads (`--keep-replies` keeps those), their attachment rows, mention rows and
    profile, in one SQLite transaction; files are deleted only when nothing else
    references them. Prints a summary.
  - `--clear-all`: deletes `~/.acp/ai/data/rs.database` and `~/.acp/ai/data/attachments/`;
    `preferences.toml` and exports are kept.
  - `--dry-run` previews; `-y`/`--yes` skips the prompt. Without `--yes`, a
    non-interactive stdin is refused; `--clear-all` requires typing `yes`.
- **Packaging, always built from source** (see `docs/INSTALL.md`)
  - Arch: `packaging/arch/PKGBUILD` (`makepkg -si`; remove with `pacman -R`).
  - Debian and derivatives: `debian/` source package (`dpkg-buildpackage`; `apt remove`).
  - Fedora/RHEL and derivatives: `packaging/fedora/acp-red-star.spec` (`rpmbuild -ba`, SRPM; `dnf remove`).
  - `scripts/package.sh` builds `.deb`/`.rpm`/`.pkg.tar.zst` into `dist/`, using host tools or a
    debian/fedora/archlinux container (podman/docker).
  - `scripts/install.sh` detects the distro (`ID`/`ID_LIKE`), installs build dependencies,
    checks for Rust >= 1.88 (preferring rustup over old distro Rust), compiles locally and installs the
    native package; `--user` installs into `~/.local` with a manifest.
  - `scripts/uninstall.sh` removes the package **and purges `~/.acp` data by default** after one
    confirmation listing everything (`--yes` to skip it, `--no-purge` to keep data). Package managers alone
    never touch home directories.
  - Packages ship a man page and bash/zsh/fish completions (hidden `--generate` flag, clap_mangen /
    clap_complete) plus docs and license.

### Changed
- `rust-version = "1.88"` declared in `Cargo.toml` (edition 2024 needs 1.85, but the `scraper`
  dependency uses let-chains, stable since 1.88); `dist/` is git-ignored.

## [1.0.0] - 2026-10-15

First release of the ACP Red Star (acp.us/rs) scraper.

### Added
- **Login**: email + one-time-code sign-in over Phoenix LiveView, finishing with the
  `/auth/user/otp/sign_in` form POST; the email is remembered in `~/.acp/preferences.toml`.
- **Browser identity**: requests use Firefox headers and a persistent cookie jar.
- **Scraping**
  - `--nick` / `--nicks @a,@b`: scrape one or many members in one session.
  - Full profile feed paging via the site's `page-next` event (LiveView 1.2 replies).
  - Thread replies stored as children (`parent_post_id`) of their post.
  - Member profiles (display name, bio, avatar/banner, counts, join date).
  - Mentions (on by default, `--no-mentions` to skip): posts mentioning a nick by
    @mention, bare nick or display name, verified against the full thread;
    `--crawl-limit` bounds the site-wide crawl.
  - `--latest` (default) and `--trending` feed modes.
  - Incremental scrapes that stop at already-stored posts; `--full` forces a re-crawl.
  - Attachments (images, GIFs, video, audio) downloaded with posts; `--no-attachments`.
- **Storage**: SQLite at `~/.acp/ai/data/rs.database` with `posts`, `attachments`,
  `mentions` and `profiles` tables; re-runs never duplicate rows.
- **Search and sharing**
  - `--search` queries the local database offline, with `--nick`/`--nicks` filters.
  - `--format json|html`: JSON (default) or a self-contained HTML page.
  - `--output`, `--name`, and `--compress --tar-gz` for shareable archives.
  - `--export` / `--import` round-trip data between databases (export format v2,
    v1 still importable); `--import` uses `--compress --tar-gz` to unpack archives.
  - `--list-nicks` shows the members already in the database.
- **CLI**: grouped `--help` with examples, `--version`, `--quiet`, `--verbose`, `--debug`.
- **Docs**: `docs/USAGE.md`, generated API docs in `docs/api/` (`scripts/gen-docs.sh`).
- **Safety**: `.githooks/pre-commit` blocks credentials and secret files from commits;
  `build.rs` enables it automatically via `core.hooksPath`.
- **License**: GPL-3.0-or-later.

### Known limitations
- Paging through very long threads and some profile fields are not yet verified live.
- Mentions are only found within the crawled window (`--crawl-limit`).
