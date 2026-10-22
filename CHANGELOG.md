# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [3.50.0] - 2026-10-22

### Added

- **Password change.** `--change-password` asks for the current password and a new one, then rewraps the database bootstrap, the node keystore, the recovery phrase, the existing master secret, and the backup wrapping key in one step. A failure puts the old wraps back. A wrong current password writes nothing. The SQLCipher key stays the same. The master, the account id, and node certificates stay as they are. A backup written before the change still needs that password. A backup written after it is format 2: it opens with the new password, and a new machine can still check the phrase against the same account id.

## [3.35.14] - 2026-10-22

### Fixed

- **Recovery question.** A pending recovery phrase asks inside the terminal UI. Not now is the default and opens the recovery topic. Create leaves the screen, runs the phrase setup, and comes back. The headless prompt is unchanged. `?` still opens the key list.

## [3.35.13] - 2026-10-22

### Fixed

- **Basket export name.** Exporting the basket again leaves an existing `exports/*.html` in place and writes `{stem}-{date-time}.html`. A later collision uses `-2`, then `-3`. Other exports still replace the file they were given.

## [3.35.12] - 2026-10-22

### Fixed

- **Research file path.** Adding a file shows `~/…` or a path relative to the working directory. The path that is stored, and the path `--file` uses, stay the real path.

## [3.35.11] - 2026-10-22

### Fixed

- **HTML export Markdown.** Post bodies and research notes render as HTML. A heading, a fenced block, and a table are elements. Raw HTML in the text is escaped.

## [3.35.10] - 2026-10-22

### Fixed

- **Help layout.** Searching Help for layout finds the layout topic: Nested, Stack, Stack + content pane, Carousel, and Grid. `?` on those View choices opens it.

## [3.35.9] - 2026-10-22

### Fixed

- **Help opens on the keys.** `?` starts on the key list. The recovery phrase is the `recovery` topic. A pending recovery still opens that topic. `--help` still prints the guide.

## [3.35.8] - 2026-10-22

### Fixed

- **Neutral brand wording.** A brand that is not ACP drives the Help intro, the Brands topic, `--help`, About, and Bookmarks. Sync with acp.us and the ACP platform tab stay on the ACP brand. `--list-brands` prints the slogan only on an ACP row. Theme-preview samples follow the brand: May Day, Workers of the world, Strike fund, and Solidarity stay on ACP; any other brand uses the ordinary notes.

## [3.35.7] - 2026-10-22

### Fixed

- **Thread sort offline.** `s` in an open thread reorders the replies already stored on this machine. It does not start a job, and it does not open the login window. A refresh from the site is still `--sort-thread`.

## [3.35.6] - 2026-10-22

### Fixed

- **Help search.** Letters typed in Help go into the search, including j and k. ↑ and ↓ scroll. Searching for basket shows basket.

## [3.35.5] - 2026-10-22

### Fixed

- **Theme preview brand.** The theme editor's preview uses the brand that is on screen, and the sample nicks @sample and @reader. A neutral brand no longer shows the ACP Red Star header, logo, or motto in that preview.

## [3.35.4] - 2026-10-22

### Fixed

- **Theme preview samples.** The theme editor's sample posts are @sample and @reader, with ordinary preview text. Headings, code, a table, a quote, and a footnote are still there so the preview can show them.

## [3.35.3] - 2026-10-22

### Fixed

- **Nick bar collapse.** ‹ collapses the nick bar, and so does a click on the cells beside it or on the bar's right edge. A nick row is still a nick. j, k, and Tab are unchanged.

## [3.35.2] - 2026-10-22

### Fixed

- **Explore timeout.** Explore waits 12 seconds for one LiveView reply, not for the whole crawl. It prints `Explore: requesting` and then `Explore: still waiting`. A timeout stores nothing. A silent join no longer stays open just because heartbeats are answered.

## [3.35.0] - 2026-10-22

### Added

- **Brands.** The logo, the title, and the motto follow a brand, the same way colours follow a theme. The built-in `default` brand is still ACP Red Star: the red-star logo, the word ACP, the title, and "The future belongs to the working class". The built-in `plain` brand is the title Red Star, a star, and no motto (`acp-red-star --brand plain`). `--brand NAME` selects one for this run. `[ui] brand` in `~/.acp/preferences.toml` keeps it. The header switch `«name» ▾` opens the picker; moving the highlight previews it and Esc puts the old brand back. The brand editor edits the title, motto, word, and logo (Ctrl-S save, Ctrl-R revert, Ctrl-N save as new, Ctrl-D duplicate, Ctrl-X delete, Ctrl-C close). User brands are `~/.acp/brands/<name>.toml`. A logo is built-in, inline text, or a file inside that folder; a path that leaves the folder is refused. A brand is independent of the theme. `--help`, `--version`, the terminal title, and HTML export headers use the title. Credits and the other surprises keep their own text.

## [3.34.0] - 2026-10-21

### Added

- **Drafts and the outbox.** A post is stored on this machine before it is sent. `--draft-save` writes a draft. The same `--draft-id` writes another `post_versions` row. `--keep-local` leaves the post off the queue. `--draft-list` prints them. `--outbox-queue ID` queues that local id. `--outbox-list` prints the queue. `--outbox-cancel ID` marks a queued, failed, or draft row cancelled. `--outbox-send ID` asks `Send from the outbox? [y/N]`. `--yes` confirms. `--dry-run` prints the same validate and save frames as a direct text publish and opens no connection. An item older than 24 hours asks again. A row left in `sending` is read back before another try. The queue sends only a text post or a reply. Votes and the other gated writes are not sent from it. In the terminal UI, Save draft is on the compose box and under Compose. Send from outbox starts on Cancel.

## [3.33.0] - 2026-10-21

### Added

- **Reply edit.** A confirmed reply edit sends validate, then save, once, from `--edit` and the compose box. The open is the captured redirect to `/rs/post/ID/edit`. The form is `method=put`, `data-mode=reply`, and the only field is `form[body_input]`. `--body` or `--body-file` is required. `--dry-run` prints those frames and opens no connection. The command line asks `Edit? [y/N]`, then `Type yes`. The compose box starts on Cancel, then asks for `yes`. The one-hour window still refuses an older post before a connection. A quote already on the reply is not copied. Votes, subscribes, deletes, reposts, and top-level post edits stay prepared, gated. The site does not support changing attachments on an edit. `--attachment` with `--edit` is refused before a connection.

## [3.32.0] - 2026-10-21

### Added

- **Attachment uploads.** A confirmed new post or reply uploads the site copy, then sends validate and save once, from `--new-post`, `--reply`, and the compose box. The site copy is at most 4 images, videos, or PDFs. Audio and other documents stay on this machine and are named in the result. The frames are the 2026-10-21 capture: `allow_upload`, an `lvu:<entry ref>` join, binary `chunk` frames, then `progress`, then save. A rejected file stops the publish before save. A failed chunk is not retried. The command line and the status line show `uploading name N%`. `--dry-run` prints the plan and opens no connection. Votes, subscribes, deletes, edits, and reposts are not sent.

### Changed

- **Live text and image publish.** Michael confirmed a text post with one PNG from commit `61c58300`, at the `Publish? [y/N]` prompt, without `--yes`. The client printed `Already logged in.`, `uploading social-red-star.png 100%`, and `published AaFe-qikeQyiWLMA1vwnNA` (`https://acp.us/rs/post/AaFe-qikeQyiWLMA1vwnNA`). A PDF upload and a video upload have not been tried live. The note is [docs/LIVE-PUBLISH-TEST.md](docs/LIVE-PUBLISH-TEST.md).
- Working rules for agents are in `AGENTS.md`. The X and YouTube design is in `architecture/PLAN-SCRAPERS.md`. Neither is a feature of this version.

## [3.31.0] - 2026-10-21

### Added

- **Text publish.** A confirmed text post or reply sends validate, then save, once, from `--new-post`, `--reply`, and the compose box. The reply frames are the approved capture, including a comment or quote reply. A new post sends `form[title]` and `form[body_input]`. Success prints the new post id and URL. A rejection is shown and is not retried. `--dry-run` prints the frames and opens no connection. The command line asks `Publish? [y/N]`. The compose box starts on Cancel. The 30 second gap starts on confirm. Uploads are 3.32.0. Votes, subscribes, deletes, edits, and reposts are not sent.

## [3.30.0] - 2026-10-21

### Added

- **Whole site.** `acprs --scrape-all` reads every reachable read-only page: feeds, threads and replies, profiles, tags, spaces and communities when the page links them, reposts, and attachment URLs and types. The file bytes are not downloaded. The frontier is stored in the encrypted database, so a stopped run resumes. `--limit` caps new posts. `--scrape-budget` caps seconds. Pages wait, back off after a refusal, and honor `robots.txt`. The next finished run is incremental unless `--full`. Scrape › Whole site and Sync › Whole site start it. Nothing is published.

## [3.29.0] - 2026-10-20

### Added

- **Alert templates.** Sync › Rules presets watch a user (`w`, `--rules-watch`), a saved search (`s`, `--rules-saved`), and replies to me (`m`, `--rules-replies`). Each is a user rule on the rule engine, enabled, priority 40, action notify. The notice names the author, the post id, and the site URL. There is no separate alert runner.

## [3.28.0] - 2026-10-20

### Added

- **Rule engine.** Sync runs triggers, conditions, and actions on the rows it just wrote. The shipped Synced post rule is off. System rules are read-only and can be cloned into a user rule. Sync › Rules edits one condition and one action. `--rules-list`, `--rules-show`, `--rules-enable`, `--rules-disable`, `--rules-clone`, `--rules-delete`, `--rules-priority`, `--rules-import`, `--rules-export`, and `--rules-preview` are the command line. Import and export are TOML and JSON. A preview matches posts already stored and applies nothing. A post fires at most once per rule. Notify prints a line and does not raise a toast. Nothing is sent to the site.

## [3.27.0] - 2026-10-20

### Added

- prepared, gated: vote. `+` and `-`, and `--vote`, stop at the capture gate.
- prepared, gated: subscribe. The subscribe hook stops at the capture gate.
- prepared, gated: delete. A confirmed delete stops at the capture gate.
- prepared, gated: edit. A confirmed edit inside the one-hour window stops at the capture gate.
- prepared, gated: repost. A confirmed repost stops at the capture gate.
- **Notices.** An error, a warning, a skipped sync, or a finished job raises a stacked toast and a desktop notification. When the stack would be taller than a quarter of the screen, as many toasts as fit stay up and View all opens the rest. View › Notifications is that page: newest first, with oldest, a kind filter, search, a clear button on each row, and Clear all. The rows are stored in the encrypted database. Do not disturb and a switch for each kind silence the toast and the desktop notification together. The list still records them. The same notice prints on stderr. Tests use a fake desktop backend.

## [3.26.1] - 2026-10-20

### Fixed

- **Default theme download.** The first-run download and `--theme-init` fetch `themes/default.toml` from `m3talsmith/acp-rs-scraper`, the repository that holds the file. The old URL used `m3talsmith/acp-red-star`, which is not this repository, so the download failed.

## [3.26.0] - 2026-10-20

### Added

- **Reply frames match the approved capture.** A root reply starts with a `postRef` node whose `postId` and `threadId` are both the thread id, then one paragraph. A blank line is two `hardBreak` nodes inside the paragraph. The form value is `form[thread_id]` then `form[body_input]`, with no `_csrf_token`. Validate sends `meta._target` of `form[body_input]` and `uploads: {}`. Save sends an empty `meta` and no `uploads` key. Reply in the terminal UI (`r`) and `--reply` both build these frames. `--dry-run` prints them and opens no connection. Sending them is 3.31.0.

## [3.25.0] - 2026-10-20

### Added

- **Action history.** The right bar lists every action that led to the current view and collapses to one column. The list scrolls. Clicking an earlier action jumps back. A new action after a jump keeps the later steps as another branch. The bar shows which branch, and `b` lists them. Branches are stored in the encrypted database. Clear history deletes them. The nick bar collapses the same way.

## [3.24.0] - 2026-10-20

### Added

- **Search history.** Each local or site search is a row in the encrypted database: the query and the filters only, including order and limit. Picking one re-runs it. Clear history deletes those rows and leaves posts alone. The default keeps the entire history. Search › History kept can cap it at the N newest rows. An export of a search adds a `search` object. Format version stays 2. A full dump, a nick filter, and a basket do not add one.

## [3.23.2] - 2026-10-20

### Fixed

- **Terminal UI login, follow-up.** A Phoenix heartbeat that is written but not answered within one interval is a dead socket, and the login reconnects without sending another email. The sign-in POST uses the code form from the LiveView diff: `user[otp]`, `submit-code`, that form's `_csrf_token`, `user[remember_me]`, `marketing_opt_in` when the form has it, and `return_to` from the `/auth` URL. A redirect back to `/auth` is a rejection and the page's flash is the error. The login window names the real session file, including when `ACP_HOME` is set. The one-time code is not written to the debug log.

## [3.23.1] - 2026-10-20

### Fixed

- **Terminal UI login.** Entering the emailed code no longer fails with "websocket closed". The sign-in POST kept a non-`Send` HTML node alive across the request, so the terminal UI ran login on a second runtime and that split closed the LiveView socket. The form fields are copied first and the login runs on the same runtime as the HTTP client. A reconnect closes the old socket before opening another. The session is written to `~/.acp/session.json`. `ACP_HOME`, when set and not empty, is the directory that holds `.acp` instead of `$HOME`.

## [3.23.0] - 2026-10-20

### Added

- **About.** About is a menu with three segments: Credits, Why this is necessary, and ACP platform. Each opens the same modal on that section. ACP platform is the vendored American Communist Party Program (adopted October 12, 2024), with headings and scroll. `https://acp.us/join` sits at the top and the bottom of that section only.

## [3.22.0] - 2026-10-20

### Added

- **Help and Getting Started.** Help is topics you can search and step through with ← →. `?` and F1 stay. A `?` in the header corner opens Help. Getting Started is `docs/GETTING-STARTED.md` and the `getting-started` topic, covering the command line, terminal prompts, and the TUI. A tooltip's second line opens that action's Help topic.

### Changed

- Crate 3.22.0 is this Help work. The plan's re-key stage was also called 3.22.0. That re-key has not shipped. It is not this version.

## [3.21.0] - 2026-10-20

### Added

- **Hover tooltips.** Hover shows a short explanation on every button, menu entry, and card icon, including the key when the control has one (`Bookmark (b)`). The second line is `? {topic}`. Star, basket, and research do not share a topic. One helper draws every tooltip.

## [3.20.0] - 2026-10-20

### Changed

- **Menu bar categories.** The bar is View, Search, Compose, Library, Bookmarks, Research, Sync, Discover, Account, Debug, Help, About, and Quit. Scrape is pinned immediately left of Start Scanning. Library opens Export, Import, Basket, List, and Maintain. Quit stays the last menu item. About stays beside Help. Discover says people, nodes, and rooms arrive with the network node. Search › Search the site runs `--site-search`.

## [3.19.1] - 2026-10-20

### Fixed

- **Clippy is clean.** `scrape` takes a `ScrapeOptions` value and `read_view` takes a `ViewRequest`, instead of a long argument list. Nothing a person sees in the terminal UI changes.

## [3.19.0] - 2026-10-20

### Added

- **`acp` theme.** A shipped theme matching the American Communist Party site (acp.us). The light variant is blush `#fff1f1`, black `#000000`, and ACP red `#c01c1c`. The dark variant is black with blush text. There is no gold. `acp` is in the theme picker once the file is in `~/.acp/themes`. `--theme-init acp` downloads it.

## [3.18.0] - 2026-10-20

### Added

- **`rs` theme.** A shipped theme matching the Red Star site (acp.us/rs): near-black zinc, ACP red `#c01d1c`, and vote gold `#eaa313`. The light variant is zinc grey with the same red and a darkened gold. `rs` is in the theme picker once the file is in `~/.acp/themes`. `--theme-init rs` downloads it.

## [3.17.0] - 2026-10-20

### Added

- A few surprises for the curious. The notes are in `docs/.secrets/EASTER_EGGS.md`. They are not in the README, usage, or Help.

### Changed

- The 4.0 roadmap, post source, and the Haz stream corpus are recorded in `architecture/PLAN-4.0.md` and `architecture/PLAN-3.0-CLIENT.md`. No scraper and no schema change are in this version. An embedded local assistant is 4.0, not a 3.x requirement.

## [3.16.0] - 2026-10-20

### Changed

- **`red-star` theme.** The shipped theme matches the brand banner: ink `#0e0404`, cream `#f6e8c8`, vermilion `#c8102e`, gold `#e8b648`, with a light variant on cream. It replaces the old deep-crimson palette. The name stays `red-star`. A copy already saved in `~/.acp/themes` is not overwritten.

## [3.15.0] - 2026-10-20

### Added

- If you go looking, there is art in the dark of the tree. It is not installed with the package.

## [3.14.0] - 2026-10-20

### Added

- **`acprs`.** A third binary, next to `acp-red-star` and the `acp-rs-scraper` compatibility name. `acprs` is its own executable and runs the same program. `--version` still prints `acp-red-star` and the version. The usage line uses the name you typed. `acprs` does not print the deprecation line. Packages and `scripts/install.sh` install it with a man page and completions.

### Changed

- The README uses the ACP Red Star brand banner.

## [3.13.0] - 2026-10-19

### Changed

- **The product name is ACP Red Star.** The binary is `acp-red-star`. `acp-rs-scraper` still runs, prints one deprecation line, and forwards its arguments through version 3. The library crate is `acp_red_star`. The shared core is `acp_red_star_node_core`. The encrypted database stays at `~/.acp/ai/data/rs.database`. The terminal UI shows the slogan "The future belongs to the working class" under that title on the welcome screen and the login screen, as the header of About, and at the right of the status bar when the bar is wide enough.
- The coming-soon draft is `docs/history/v3/redstar-v3-prelaunch-post.md`. The launch post, written as if 3.x has fully shipped, stays unpublished until 3.x is done.
- Alpha packaging stays parked until writes and replies with attachments are sent and the decentralized network work is done. The server and the relay each stay a separate install on that same gate. This version does not tag an alpha.

## [3.12.1] - 2026-10-19

### Fixed

- **Compressed export paths.** With Compress (tar.gz) on, a path that does not already end in `.tar.gz` or `.tgz` is no longer used as both the archive and the staging directory. A bare name gets `.tar.gz` appended. A directory, or a path ending in `/`, is `<dir>/<export-name>.tar.gz`. The success line is `wrote <final path>`.

## [3.12.0] - 2026-10-19

### Added

- **`--color auto|always|never`.** `auto` (the default) follows `NO_COLOR`. `always` forces colours on. `never` turns them off. `[ui] color` in `preferences.toml` is the default when the flag is omitted. View › Colour sets the same choice.

## [3.11.0] - 2026-10-19

### Added

- **TUI path fields expand `~` and Tab-complete.** Export › Path, Import › Path, Research › Export to, Research › Local file, and Start Scanning › Output file complete one real path segment on Tab, the same way the compose attach line does. The argv the child runs expands `~` and `~/…`.

## [3.10.0] - 2026-10-19

### Added

- **Site copy takes up to 4 images or videos.** The app, the local archive, and the network keep every file. Red Star accepts `image/*` and `video/*` only, at most 4, on the site copy. Audio, documents, and other types stay local. More than 4 images or videos, with no choice, publish the first 4 of those and print a notice naming the ones left off. `--site-attachments` takes 1-based indexes. The compose box opens a picker when there are more than 4. Each stored file has `on_site`. Export JSON adds `"on_site"`. A confirmed publish still does not send. PDFs join this cap in 3.32.0.

### Changed

- `.jpeg` is accepted as jpg and `.tif` as tiff. There is no count cap and no size cap on the app, the archive, or the network. The site copy is the separate cap of 4.

## [3.9.1] - 2026-10-19

### Fixed

- **Terminal UI login.** Entering the emailed code no longer fails with "websocket closed" and then drops the attempt. The login keeps the LiveView socket alive with Phoenix heartbeats while the code is read, reconnects and retries verification if that socket closes, and stays on the code step when the code is wrong. Resend code (F2) asks for another code on the same login. A finished login is written to `~/.acp/session.json`. The command line also heartbeats while it waits at the prompt.

## [3.9.0] - 2026-10-19

### Added

- **Order and limit in the terminal UI.** Every post list has an order, newest or oldest. `o` toggles it. View › Order sets it. Each list remembers its choice under `[ui.list_order]`. Search and the nick filter also take a limit, the same window as `--order` and `--limit`.

## [3.8.0] - 2026-10-19

### Added

- **`--profile-feed` and `--order` on a nick scrape.** `--profile-feed newest` (the default) is the Latest profile. `--profile-feed top` is `/rs/@nick/top?t=all` only. `--order oldest` keeps the end of that list, so `--limit` is the oldest N. Start Scanning has Profile feed and Order.

## [3.7.0] - 2026-10-19

### Added

- **`--limit N` on a nick scrape.** `acp-red-star --nick alice --limit 100` keeps the newest 100 posts from that profile. Omit it to scrape the whole profile. Replies in those threads are still fetched unless `--said-by`. `--crawl-limit` still only bounds the mentions scan. Start Scanning has the same field.

## [3.6.0] - 2026-10-19

### Added

- **Tab-complete on the compose attach line.** Tab on a non-empty line completes the next real path segment (`~` becomes `~/`). Several matches are listed. Tab, Up, and Down move through them, and Enter writes the highlighted one without attaching yet.

### Changed

- A version that is not on `main` is a draft GitHub Release, with the notes from its changelog section and no attached files. The maintainer publishes the draft after that version is merged. The post-commit hook still only creates the git tag.
- A signed tombstone keeps the post. The plan no longer drops the body after a grace period. The UI shows a tombstone icon (🪦, or `[RIP]`), the content stays viewable, and a signed restore record or a newer signature of the same post id clears the tombstone.
- Password change / re-key is a plan stage. It does not change the master or the `acp-id`. It re-wraps the SQLCipher key, the keystore, the sealed phrase, and the backup-wrapping key in one transaction. It has not shipped. Crate 3.22.0 is Help, not this re-key.

## [3.5.0] - 2026-10-19

### Added

- **Attachments on New post and Reply.** The compose box has an attach line: type a path and Enter. Backspace on an empty line removes the last file. `--attachment PATH` repeats on `--new-post` and `--reply`. Media is avif, gif, jpg, jpeg, png, tif, tiff, webp, mkv, mov, mp4, or webm. A pdf is a file attachment. `--dry-run` lists each file (kind, name, size, and path) and does not upload it.

### Not sent

- **The upload channel is not captured.** `allow_upload` and `lvu` are not built, so `media_attachments` and `file_attachments` are not encoded and no file bytes are sent. A confirmed publish still stops where 3.4.0 stopped: `validate` and `save` are not sent. Vote, subscribe, delete, edit, and repost are not in this release.

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
