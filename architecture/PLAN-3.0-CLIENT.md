# Plan: acp-rs-scraper 3.0, a full Red Star client

Status: **3.0.0 is implemented** (identity, encryption at rest, backup and restore). **3.1.0 is implemented** (job details). **3.2.0 is implemented** (shared `acp_rs_node_core`). **3.3.0 is implemented** (local node identity). Later phases in this file are still design.
Base: 2.16.1 (`55fd11cd`); 2.17.0 (full Markdown rendering) is in progress. Companion docs: [SPECS.md](./SPECS.md) and [PLAN-2.0-TUI.md](./PLAN-2.0-TUI.md).

Update 2026-10-18: §7.13 (keys, identity, encryption at rest, backup and recovery) and §7.14 (onboarding, operations, crates, Dart SDK, local-first posts and review)
were added after review with Michael. §7.13 supersedes the single node identity key in §7.2, and §7.14 wins wherever it disagrees with an earlier section.
Their decisions are marked resolved in §8 Q11 and Q12.

Update 2026-10-18 (versioning): **3.0.0 is identity, encryption at rest, and backup/restore**, not the write core. It is a breaking change for a 2.x plaintext database. The write core and every later phase move up one minor (§4, §7.11, §7.14.9). Authorizer and node certificates, enrollment, revocation, relays and peers stay in P9 (3.23.0 and later).

Update 2026-10-19: **3.1.0 is the job-details window** (click a running job in the terminal UI). The write core moves to 3.2.0, and every later phase moves up one minor with it.

Update 2026-10-19 (crate split): **3.2.0** is `acp_rs_node_core`: identity crypto, canonical CBOR, and a secret-store interface, separate from the terminal client. **3.3.0** is local node identity (an independent node key, how a node is named and verified, the certificate format, `--node-init` and `--node-id`). The write core moves to **3.4.0**, and every phase the 3.1.0 update numbered from 3.2.0 upward moves up two minors. Writes stay off until 3.4.0. Network enrollment, relays, and peers stay in 3.23.0 and later. There is no 4.0.

Update 2026-10-19 (write core): **3.4.0** ships compose for New post and Reply, the Markdown to ProseMirror writer, and the safety layer (writes on by default, Cancel first, `--dry-run`, the 30 second gap, bundle-hash refusal). Publish does not open a connection. The P0 capture has not confirmed the submit payload, so validate/save, `_target`, uploads, mention-query replies, and read-back stay stubbed. Attachments wait for 3.5.0. Later write features stay in their own minors.

Update 2026-10-19 (decisions): an authorizer certificate lasts **30 days**, a node certificate lasts **365 days**, and clock skew is **5 minutes**. Renewal, the 7-day grace period, and revocation stay in the stages already planned for them. The first machine's authorizer limits are accepted (`history_grant = allowed`, `max_nodes` 16). A future daemon may unlock from the OS keyring through `SecretStore`. Public nodes exist, as an opt-in on the serve and relay work. APT, COPR, AUR, and the Homebrew formula stay unpublished until the full client set exists.

Update 2026-10-19 (tombstones and password): a signed tombstone **keeps** the post. The UI shows it as deleted with a tombstone icon, and the body stays viewable. A later signed post or a signed restore record for that post id clears the tombstone. A password change does **not** change the master or the `acp-id`. **3.22.0** is that re-key, inserted before the daemon and keyring work, so every stage that was 3.22.0 or later moves up one minor. There is still no 4.0.

Update 2026-10-19 (TUI menu, required before 3.x is finished): the menu bar is reorganized into the categories in §4.2. This is a hard requirement for 3.x, and it is its own minor. It is not polish after 3.x. This note does not assign that minor a number and does not move stages that already have one. 3.5.0 attachments does not include it. Start Scanning stays pinned on the right. **Bookmarks** is its own top-level menu, not an item under Library. **Sync** keeps the local copy up to date. **Discover** finds people, nodes, and rooms. Those two menus stay separate. Hover tooltips on every actionable control are the same kind of requirement (§4.3). Friendlier, searchable Help, with a help icon on the chrome, is the same kind of requirement (§4.4). Each can ship with that menu minor or as its own.

Goal: the app can do everything the Red Star website (`https://acp.us/rs`) can do,
plus two client-side features the site doesn't have: **Feeds** (curated, shareable views)
and **decentralization** (nodes that replicate their databases peer-to-peer over libp2p).

---

## 0. How this inventory was made (survey notes)

- **Survey date:** 2026-10-18. Every request was a GET, sent with a *copy* of `~/.acp/session.json`
  in a throwaway directory. That directory has since been deleted, and the real `~/.acp` wasn't touched.
- **Nothing was written to the site.** No forms were submitted and no LiveView events were pushed.
  No websocket was opened, so `view-post` view tracking never fired.
- **Sources:**
  - Server-rendered HTML for 20 pages: `data-props` keys only, plus `phx-*` attributes and forms.
  - The JS bundles: `/assets/app-*.js`, `/assets/svelte-*.js` and 40 Svelte/Vite chunks
    (Post, Feed, Thread, Editor, Edits, SpaceBar and so on), searched for
    `pushEvent`/`pushEventTo`/`handleEvent` and for route strings.
  - The existing code in `src/auth`, `src/liveview`, `src/scrape` and `src/bookmarks`.
- **Pages deliberately not opened:**
  - `/rs/notifications`, because it might mark notifications read. (Now allowed, resolved 2026-10-18: P0 opens it; see §4.1.)
  - `/sign-out`, because a GET logs out.
  - `/rs/post/{id}/edit`, because Michael has no own posts on his profile to open safely.
  - Anything under `/party-admin`.
- **Stack:** Phoenix LiveView 1.x (the websocket at `/live/websocket?vsn=2.0.0`), with
  `live_svelte` components mounted through `phx-hook="SvelteHook"` and JSON `data-props`.
  - Bodies are **TipTap/ProseMirror JSON**.
  - Uploads use LiveView uploads (`Phoenix.LiveFileUpload`, `data-phx-upload-ref`).
  - Spaces are live audio/video over **LiveKit**.
  - Web Push goes through a service worker.
- **No JSON REST API was found.** Every action is a LiveView event, either a form `phx-submit`
  or a hook `pushEvent` with a reply callback.

### What the app already uses (2.16.0)

| Area | Mechanism |
|---|---|
| Login | `GET /rs` redirects to `/auth`. The LiveView form event `send-login-email` (`user[email]`, cid 1) sends the email, then the code form (`verify-code` / `user[code]`, detected at runtime). After that comes either a redirect or the `phx-trigger-action` POST to `/auth/user/otp/sign_in` with `_csrf_token`. The `_acp_key` and `remember_me` cookies are saved to `~/.acp/session.json` (0600). |
| CSRF | `meta[name=csrf-token]` is sent as `_csrf_token` in the websocket query and `phx_join` params. |
| Join | `phx_join` to `lv:<root id>` with `url`, `session` (`data-phx-session`) and `static` (`data-phx-static`). |
| Feeds | The `div#forum/Feed[data-props]` props (`posts`, `next_cursor`, `at_end`, `feed_key`, `live_action`, `pinned_post`, `current_user`), then the hook `page-next {cursor}`. |
| Threads | `div#forum/Thread[data-props]` (`root`, `replies`, `sort`, `canQuote`, `focused_id`), then the hook `next-page {}`. |
| Profiles | `/rs/@nick`, `?replies=1` (`configure-feed`, field `replies`), `/rs/@nick/top?t=all`, and the `show-info` hook. |
| Bookmarks | Reads `/rs/bookmarks`. Writes with the click event `toggle-bookmark {id}`, a write that's already shipped (2.5.0). **The current JS sends `bookmark {id, state:"1"/"0"}` instead.** The old name still worked at 2.5.0, but it could disappear. |
| Logout | Local only. `/sign-out` (GET) now turns out to exist on the site. |

Post JSON (the thread `root` and feed items) has these keys: `actions{admin,delete,edit,manage_space,moderator,report,repost}`,
`after_cursor`, `author{active,display_name,profile_photo_uploaded,profile_photo_url,sustainer,sustainer_yearly,user_id,username}`,
`author_location`, `author_member_role`, `body`, `body_text_summary`, `body_truncated`, `bookmarked`, `bookmarks_count`,
`deleted`, `files`, `id`, `inserted_at`, `last_edited_at`, `latest_reply`, `latest_reply_at`, `locked_at`, `media`,
`quoted_post`, `references`, `referencing`, `replies_count`, `reported`, `reposts_count`, `score`, `space`,
`subscribed`, `thread_deleted`, `thread_id`, `thread_title`, `truncate`, `upvote_percentage`, `views` and `vote`.
`current_user` has `{id, moderator?}`. For Michael, `moderator?` is false.

---

## 1. Feature inventory

The table below lists 37 site features, numbered 1–37, plus two client-side extras (Feeds and Nodes), numbered 38–39.

Legend for "We have": ✅ = done, ◐ = partial, ✗ = missing, n/a = out of scope (proposed).
"Conf." is confidence in the mechanism: **C** = confirmed in HTML/JS, **I** = inferred, **U** = unknown until a capture.

### 1a. Reading and navigation

| # | Site feature | How the site does it | Conf. | We have | TUI design |
|---|---|---|---|---|---|
| 1 | Trending home | `/rs` with `forum/Feed`, `page-next {cursor}`, `refresh {}` | C | ✅ | Existing |
| 2 | Latest | `/rs/latest` | C | ✅ | Existing |
| 3 | Top posts (site-wide) | `/rs/top`, with `configure-feed` and fields `t` (5 periods) + `replies` | C | ◐ (profile top only) | View-mode menu: Top › period |
| 4 | Executive Board feed | `/rs/eb` (+ `replies` filter) | C | ✗ | Nick-bar row "EB" |
| 5 | Explore | `/rs/explore` and `/rs/explore/latest` | C | ✗ | Explore screen |
| 6 | Site search | `/rs/explore?q=…` (form `phx-submit="search"`, field `q`) returns a `forum/Feed` | C | ✗ (local search only) | Search modal gets a "Site / Local" toggle |
| 7 | Tag feeds | `/rs/tag/{tag}` returns a `forum/Feed` (same pagination) | C | ✗ | Click a `#tag` to open its feed |
| 8 | Thread view + focus | `/rs/post/{thread}` and `/rs/post/{thread}/{post}` (patch) | C | ✅ | Existing |
| 9 | Thread sort | hook `sort {sort:"oldest"\|"top"}` | C | ✗ | `s` in thread view |
| 10 | Profiles | `/rs/@nick`, `replies` filter, `/top` | C | ✅ | Existing |
| 11 | Profile card / username history | hook `show-info {id}` to `#js-profileInfo`, plus a "Username history" dialog | C | ◐ | Profile modal tab |
| 12 | Bookmarks list | `/rs/bookmarks` | C | ✅ | Existing |
| 13 | Reposts of a post | `/rs/post/{id}/reposts` returns a `forum/Feed` | C | ✗ | Post menu › View reposts |
| 14 | Edit history | `/rs/post/{thread}/{id}/edits` with a `forum/Edits` component (`edits`, `post`) | C | ✗ | Post menu › View edits, as a diff |
| 15 | Notifications | `/rs/notifications` (`ForumNotificationsList` hook; items `.js-notification` with `data-thread`/`data-id`). The header shows an unread count. `clear-app-badge` is pushed. | I/U | ✗ | Notifications pane (bell in header) |
| 16 | Show more (truncated) | hook `show-more {id}` / thread page | C | ✅ (refetch) | Existing |
| 17 | Rules page | `/rs/rules` (static) | C | ✗ | Help › Forum rules |
| 18 | Sustainer page | `/rs/sustainer` (list + identicons) | C | ✗ | Low priority, read-only |
| 19 | Live updates | Pushed events `thread:insert`, `thread:update-post`, `feed:delete`, `feed:refresh`, `trending-current`, `trending-latest`. The client polls `refresh-posts {ids}` for visible posts. | C | ✗ | Live badge; new replies appear in place |
| 20 | View counting | hook `view-post {dom_id}` (`trackPostViews`) | C | ✗ (on purpose) | Off by default. **Open question.** |

### 1b. Writing and interacting

| # | Site feature | How the site does it | Conf. | We have | TUI design |
|---|---|---|---|---|---|
| 21 | New post | `/rs/post/new`. Form `#js-postForm-form` (cid 1), events `phx-change="validate"` and `phx-submit="save"`. Fields: `form[title]`, `form[body_input]` (hidden; `JSON.stringify(editor.getJSON())`, a ProseMirror doc), `media_attachments` (multiple; avif/gif/jpg/png/tiff/webp/mkv/mov/mp4/webm) and `file_attachments` (pdf). Limit of 40,000 characters. Server events `saving` and `discard-draft`. | C (fields) / U (exact submit payload) | ✗ | Compose modal: title + Markdown body, converted to ProseMirror, with preview and attachments |
| 22 | Reply | The same form on the thread page (cid 2), with hidden `form[thread_id]` | C | ✗ | `r` in thread/detail opens the Compose modal |
| 23 | Reply to a comment | Inserts a `postRef {postId, threadId}` node at the start of the body (`acp:reply`) | C | ◐ (we already parse `postRef`) | `r` on a comment pre-fills the ref |
| 24 | Quote selection | `canQuote`: selected text becomes a `blockquote` + `postRef` | C/I | ✗ | `Q` quotes the highlighted lines |
| 25 | Mentions and tag autocomplete | `pushEventTo('#js-postForm', 'mention-query'\|'tag-query', {query})`. Results land in a listbox. Nodes are `mention {id, label:"@nick"}`; tags are `mention` with `mentionSuggestionChar:"#"`. | C | ✗ | Pop-up completion in the compose box |
| 26 | Repost / quote-post | `/rs/post/new?repost={id}` (the post renders `quoted_post`), gated by `actions.repost` | C | ✗ | Post menu › Repost (Compose with an embedded quote) |
| 27 | Edit post | `/rs/post/{id}/edit` (`actions.edit`). Probably the same editor form. | I | ✗ | Post menu › Edit (own posts) |
| 28 | Delete post | hook `delete-post {id}` after `confirm()` (`actions.delete`) | C | ✗ | Post menu › Delete, typed confirmation. A tombstoned post stays in the list with a tombstone icon and its body still readable (§7.5). |
| 29 | Up/down vote | hook `vote {id, state:"1"\|"0"\|"-1"}`. The post has `score`, `vote` and `upvote_percentage`. | C | ✗ | `+` / `-` keys and a score column |
| 30 | Bookmark | hook `bookmark {id, state}` (current), or the click `toggle-bookmark {id}` (what we use) | C | ✅ | Existing; move to the new event |
| 31 | Subscribe to a thread | hook `subscribe {id, state:"1"\|"0"}` (field `subscribed`) | C | ✗ | Post menu › Subscribe |
| 32 | Share / copy link | Client-side only (`navigator.share` / clipboard) | C | ◐ | `y` copies the URL (OSC 52) |
| 33 | Report post | `pushEventTo('#js-reportPostActions', 'show-actions', {post_id, thread_id})`, then form `js-reportPostActions-form` `save` (fields loaded on open; probably `form[reason]`) | C/U | ✗ | Post menu › Report, then a reason picker |
| 34 | Report user | `js-reportUserActions-form` (cid 3), `save`, `form[reason]` ∈ {constitution, illegal_obscene, spam, uncomradely} | C | ✗ | Profile › Report |
| 35 | Edit own profile | `edit-profile-form` (`phx-submit="save"`, method put): `form[display_name]`, `form[username]`, `form[forum_profile][bio]` (+ hidden `_persistent_id`/`_form_type`), uploads `profile_photo` and `banner` | C | ✗ | Account › Edit profile |
| 36 | Forum settings | `/rs/settings`: `settings-form` with `form[time_zone_id]` (345 zones), `form[theme]` (light/dark/system) and `form[show_sustainer?]`. Web-push buttons `phx:push-subscribe` / `delete-push-subscriptions`. | C | ✗ | Account › Site settings (web push n/a) |
| 37 | Spaces (live rooms) | `/rs/spaces` (a feed of space posts) and the `forum/SpaceBar` with a LiveKit URL and token. Events: `space-join`, `space-leave`, `space-raise-hand`, `space-lower-hand`, `space-react`, `space-recording`, `start-space`, `cancel-space`, `/rs/spaces/{id}/edit`, plus host tools (`space-mute`, `space-promote`, `space-demote`, `space-end` and so on). | C | ✗ | Phase 1: list spaces plus a "live now" badge. Phase 2 (optional): recordings via an external player. Live audio in a TUI is n/a. |

**Not found on the site:** follows, direct messages, polls, emoji reactions on posts
(only votes; reactions exist inside Spaces), and groups or channels (Spaces are the only "rooms").

**Out of scope (not Red Star):**
- The ACP member area `/profile`: leave party, billing, Telegram, application.
- The Sustainer pledge and checkout (Stripe).
- The blockchain membership verification (CDP wallet).
- Moderator and admin tools (`#js-moderatorPostActions`, lock, `/party-admin`). Michael isn't a moderator.

| # | Client-only extras | We have | Section |
|---|---|---|---|
| 38 | Feeds (curated, saved, exportable, shareable) | ✗ | §6 |
| 39 | Nodes / P2P replication (libp2p) | ✗ | §7 |

Coverage today: 7 of the 37 site features are fully covered (1, 2, 8, 10, 12, 16, 30) and 4 are partial (3, 11, 23, 32).
Only bookmark toggling writes.

---

## 2. Architecture changes

### 2.1 A write-capable site layer (`src/site/`)
- **`SiteClient`** owns the `reqwest` client and jar. It provides `open(path) -> LvPage`, `join(&LvPage) -> Lv`, and a long-lived `LiveSession`
  per open view, with the Phoenix heartbeat every 30 s and reconnect with backoff.
- **Typed operations** go in `enum SiteAction`:
  - `Post{title, body, attachments}`, `Reply{thread, in_reply_to, body, …}`, `Edit{post, …}`, `Delete{post}`, `Vote{post, state}`.
  - `Bookmark{post, on}`, `Subscribe{thread, on}`, `Repost{post, body}`, `ReportPost{…}`, `ReportUser{…}`.
  - `UpdateProfile{…}`, `UpdateSettings{…}`.
  - Each action knows its page, event, target cid and payload builder, so the
    **dry-run printer, the confirmation preview and the mocked tests all use the same code.**
- **Form submission:**
  - `Lv::submit(event, cid, fields: Vec<(k,v)>)` sends `type:"form"` with url-encoded values, generalising today's single-field `Lv::form`.
  - It also sends `_target` and the hidden fields read from the rendered form,
    and always runs `validate` first so the server-side changeset errors come back to the user.
- **Uploads:** implement the LiveView upload protocol. That's the `allow_upload` preflight on the input's `ref`, then either the
  `lvu:<ref>` channel with chunked binary frames, or an external uploader with a presigned URL, as the preflight reply says.
  This is **unknown** until a capture, so it's its own spike.
- **Body format:** a `markdown → ProseMirror` writer limited to the editor's schema:
  - Blocks: paragraph, hardBreak, blockquote, bulletList/orderedList/listItem, taskList/taskItem.
  - Marks: bold, italic, underline, link.
  - Inline nodes: mention (`@nick` and `#tag`) and postRef.
  - It round-trips with the existing ProseMirror → text/Markdown reader. Anything it can't express is rejected before sending.
- **Feature probes:** before any write, check that the expected form or event exists in the rendered page
  (for example, `phx-submit="save"` on `#js-postForm-form`) and that `actions.<x>` is true for that post.
  Record the JS bundle hash (via `/rs/asset-versions` or the `<script src>`) and **refuse writes, with a clear message, when the
  hash differs from the last verified one** until `--accept-site-update` is passed or the protocol self-check passes.
- **Read-back:** after every write, re-read the affected post or thread to confirm, then update the DB.
- **First use of each write type:** the reply payload is confirmed by the guided capture (§4.1). Every other write type
  (post, upload, vote, delete, edit and so on) runs its first real use through the same capture mode, with Michael
  confirming. Its redacted frames then become that action's fixtures.

### 2.2 Outbox (offline queue)
- **Table:** `outbox(id, action TEXT, payload JSON, status ∈ draft|queued|sending|sent|failed|cancelled, created_at, confirmed_at, attempts, last_error, result_post_id)`.
- **Drafts autosave** locally, like the site's `localStorage` drafts (`draft-{thread_id}`).
- **Queued items** send when online. A queued item that's more than 24 h old asks for confirmation again (configurable).
  `sending` is crash-safe: on restart the item is checked by read-back before any retry, so it can't double-post.
- TUI: an Outbox pane. CLI: `--outbox-list`, `--outbox-send ID`, `--outbox-cancel ID`.

### 2.3 DB schema additions (migrations in `storage/schema.rs`)
- **`posts`, new columns:**
  - `body_json` (the raw ProseMirror; needed for edit, quote and P2P).
  - `author_user_id`, `thread_title`, `score`, `my_vote`, `bookmarked`, `subscribed`.
  - `replies_count`, `reposts_count`, `views`.
  - `last_edited_at`, `locked_at`, `deleted`, `quoted_post_id`.
  - `source` (`site`|`peer`|`local`), `observed_at`.
- **`post_versions(post_id, version_hash, body_json, edited_at, observed_at, source)`:** edit history and the P2P record store.
- **`post_tags(post_id, tag)`:** parsed from `mention` nodes labelled `#…`, with a `#word` regex fallback for old rows.
  Indexed both ways.
- **`notifications(id, kind, actor, post_id, thread_id, created_at, read, raw)`.**
- **`subscriptions(thread_id, on, remote)`** and **`votes(post_id, state, remote)`**, mirroring `bookmarks`.
- **`outbox`** (§2.2), **`feeds`, `feed_rules`** (§6) and the **`p2p_*`** tables (§7).
- **Identity and security tables** (§7.13): `identity_certs`, `revocations`, `key_epochs`, `quarantine` and `backups`. The whole database is encrypted at rest (§7.13.6).
- **Operations, local-first posts and review tables** (§7.14): `node_roles`, `local_posts`, `draft_keys`, `draft_reviews`, `draft_comments` and `draft_suggestions`. `p2p_peers` also holds the bootstrap list and relay addresses, and replaces `peers.toml`.
- **`site_state(key, value)`:** the verified bundle hash and the last protocol check. (The writes switch lives in preferences; see §3.)

### 2.4 Live updates
- Keep one `LiveSession` for the focused view (feed or thread). Map the pushed events to `AppEvent`s:
  - `thread:insert` → append a reply.
  - `thread:update-post` → patch a post.
  - `feed:delete` → remove an item.
  - `feed:refresh` / `trending-*` → show a "new posts" chip.
- Poll with `refresh-posts {ids}` for visible posts only, at most once every 30 s.
- This runs in the headless child (the jobs model), which streams JSON lines to the TUI.
- Notifications use a background session on `/rs/notifications`. Opening it may mark them read, and Michael accepted that
  (resolved 2026-10-18). P0 records the page's shape and whether opening it clears the unread count.

---

## 3. Safety

1. **Writes are on by default from 3.4.0** (resolved 2026-10-18). They're opt-out: set `ui.writes = false` in
   `~/.acp/preferences.toml` or `ACP_WRITES=off`, and the env var wins. When writes are off, every write action is greyed out with the reason.
   Bundle-hash gating (§2.1) can still refuse writes after a site deploy. The preview, confirm and `--dry-run` below never go away.
2. **Confirmation before anything is published.** Every `SiteAction` shows a preview modal with:
   - the target (thread title or post), the rendered body, the attachments and the exact event.
   - Publishing needs Enter on "Publish" (default focus: Cancel).
   - Destructive actions (delete, username change) need the word typed out.
   - CLI writes need `--yes`, or an interactive y/N. `--dry-run` prints the exact LiveView frames
     **without opening a connection**.
3. **Rate limits:** a client-side token bucket (for example, at most 1 post per 30 s and 10 votes per minute), plus backoff on errors.
4. **No secrets in logs:** keep masking cookies, `_csrf_token`, `data-phx-session`/`static` (which carry a signed
   `user_token`) and the LiveKit `token` prop in `--debug`. Add the new frames to the masker and its tests.
5. **Tests never hit the real site:**
   - A **fake LiveView server** (tokio-tungstenite server + axum) serves fixture HTML with `data-phx-*`,
     answers `phx_join` and asserts every pushed frame.
   - Every `SiteAction` has a Gherkin scenario: "preview shown → confirm → frame sent → read-back stored".
     Every action also has a "cancel → nothing sent" scenario.
   - Under `cfg(test)`, and in the Gherkin harness, the HTTP and WS layers **panic if the host is `acp.us`**.
   - Smoke runs keep using a throwaway HOME.
6. **Idempotency:** client draft ids and read-back before retries (§2.2).

---

## 4. Roadmap

This follows Michael's rule: the minor version goes up by one per feature, a fix is a patch, and 3.0.0 is the breaking release for identity, encryption at rest, and backup/restore. 3.1.0 is the job-details window. 3.2.0 is the shared core crate. 3.3.0 is local node identity. 3.4.0 is the first write feature. One minor each after that. Writes stay off until 3.4.0.

| Phase | Version | Contents |
|---|---|---|
| P0: Protocol spike (no release) | none | **Guided capture session** (§4.1): Michael writes one real reply himself, and the LiveView frames are logged with secrets redacted. The same session opens `/rs/notifications` read-only. The results become masked fixtures for the fake LV server. No browser dev-tools export. |
| P1: Read parity | 2.17.0 → 2.24.0 | **2.17.0: full Markdown rendering of post bodies (in progress, taken outside this plan).** Then 2.18.0 → 2.24.0: Site Top (3), EB feed (4), Explore + site search (5/6), tag feeds (7), thread sort (9), reposts list (13), edit history (14). Also add `body_json` and `post_tags` columns. Switch bookmarks to the `bookmark` event (fix → patch). |
| 3.0.0: Identity and encryption | **3.0.0** | Master secret, 24-word phrase, SLIP-39 shares, pinned `acp-id`, SQLCipher encryption at rest, envelope wraps, `--encrypt-db`, backup and restore (§7.13, the parts that do not need a node). Breaking for a 2.x plaintext database. Certificates, enrollment, revocation, relays and peers stay in P9. |
| 3.1.0: Job details | **3.1.0** | Click the status bar or the header running mark (or Enter on the focused status) to open a job window: name and what started it, state, start and elapsed, stage and spinner, scrollable log, exit or error, cancel when the job is still running. Several jobs are a list. The window does not show the password or the database key. Encryption, backup and restore stay on the progress screen. |
| 3.2.0: Shared core | **3.2.0** | `acp_rs_node_core`: the master, the phrase, SLIP-39, canonical CBOR (with `docs/PROTOCOL.md` and test vectors), HPKE wraps, and a `SecretStore` interface. The terminal app depends on it. CLI flags, preferences, the SQLCipher database, and `.acpbackup` format version 1 stay as they are. The other crates in §7.14.7 are later. |
| 3.3.0: Node identity | **3.3.0** | A node key generated on the machine and certified by the master (not derived from it). Label, fingerprint, PeerId, authorizer and node certificates, `--node-init`, `--node-id`, and a header chip. No `--serve`, relay, or enrollment over the network. |
| P2: Write core | **3.4.0** | `src/site` layer, safety layer, Markdown→ProseMirror, compose **New post** + **Reply** (incl. reply-to-comment, quote, mention/tag completion). Attachments if the upload spike succeeded; otherwise they go to 3.5. |
| P3: Interactions | 3.5.0 → 3.9.0 | Vote, Subscribe, Delete, Edit, Repost/quote-post. A deleted post shows a tombstone icon (🪦, or `[RIP]` when the terminal cannot draw that glyph) and the retained body stays openable (§6.6, §7.5). |
| P4: Live | 3.10.0 → 3.11.0 | Live updates; Notifications pane |
| P5: Account | 3.12.0 → 3.15.0 | Edit profile, Site settings, Report post, Report user |
| P6: Outbox | 3.16.0 | Offline drafts and queue, plus **local-first posts, drafts and `post_versions`** (§7.14.8) |
| P7: Feeds | 3.17.0 → 3.20.0 | Feeds core + view; editor + add-to-feed modal; export; share/import (§6) |
| P8: Spaces (optional) | 3.21.0 | List of spaces / live badge / recordings link |
| 3.22.0: Password change / re-key | **3.22.0** | `--change-password` re-keys local data only. The master and the `acp-id` do not change, and node certificates stay valid. The SQLCipher database key, the sealed keystore, the sealed phrase or entropy, and the backup-wrapping key are re-wrapped in one transaction that rolls back on failure. The old password is required (§7.13.13). This is before the daemon and the OS keyring. |
| P9: Decentralization | 3.23.0 → 3.27.0 | Network node: `--serve`, enrollment, relays, peers, replication, NAT traversal, write relay to the site, media blobs, feeds/research sharing (§7). The master, phrase, shares, encryption at rest and backup shipped in 3.0.0. The shared core shipped in 3.2.0. Local node identity and the certificate format ship in 3.3.0. Password change / re-key is 3.22.0, before this phase. It stays inside 3.x (§7.14.10). |
| P10: Review and Dart SDK | 3.28.0 → 3.29.0 | Draft review links, comments and suggestions (3.28.0). `acp_rs_node_ffi` and the Dart SDK (3.29.0). See §7.14. |
| TUI menu categories | later minor, required before 3.x closes | Reorganize the menu bar (§4.2). Must ship before 3.x is finished. The version number is not assigned here, and numbered stages are not moved. |
| Hover tooltips | later minor, required before 3.x closes | Every button, menu item, and card icon explains itself on hover, including its key when it has one (§4.3). One shared helper. Can ship with the menu reorganization or on its own. The version number is not assigned here, and numbered stages are not moved. |
| Help topics | later minor, required before 3.x closes | Help becomes topics you can move between, with search, and a help icon on the chrome that opens the matching topic (§4.4). `?` and F1 stay the entry. Can ship with the menu reorganization or on its own. The version number is not assigned here, and numbered stages are not moved. |

### 4.1 P0 guided capture procedure (resolved 2026-10-18)

This is a hidden developer flag, `--capture-reply THREAD` (compiled in by default, documented only in `architecture/`). It runs a
single, supervised session that ends with exactly one real reply, written by Michael:

1. **Throwaway HOME.**
   - The tool creates `mktemp -d` (mode 0700) and copies **only** `~/.acp/session.json` into it (0600).
   - It sets `HOME` to that directory for the whole session, so the real `~/.acp` (where auto-sync is on) is never read beyond
     that one copy, and never written.
   - On exit, the directory is wiped. The only thing kept is the redacted capture, after Michael says yes.
2. **Frame logger.** A tap on `Lv::push`/`recv` and on the HTTP client writes `capture.jsonl` (0600):
   - What it records: every websocket frame in both directions (`[join_ref, ref, topic, event, payload]`), plus each HTTP request's method, path, status and redirect chain.
   - **What it redacts before anything touches disk:**
     - cookies and `Set-Cookie`, `_csrf_token`, and the `session`/`static` fields of `phx_join`
     - `data-phx-session`/`data-phx-static` (they carry a signed `user_token`)
     - the LiveKit `token` prop, the login email, and any `SFMyNTY.…`, JWT-shaped or long base64 value
   - Every redaction rule has a unit test.
   - The repo's pre-commit credential scan and gitleaks run over the capture before any fixture made from it is committed.
3. **Read-only warm-up.** Nothing is published in this step.
   - Open the thread and `phx_join` it, then open the reply form.
   - Open `/rs/notifications`, noting whether the unread count clears.
   - Log the rendered form, the cids and the hidden fields.
4. **Michael writes the reply.** He writes it in `$EDITOR` or the TUI compose box. Nothing has been sent yet.
5. **Confirmation.** The tool shows:
   - the thread title and URL
   - the rendered reply exactly as it will appear
   - the redacted frames it is about to send (`validate`, then `save`, with `form[thread_id]` and `form[body_input]`)

   Michael must confirm (the default is Cancel; he types `send`). Cancelling sends nothing, and the reply text is not even sent to `validate`.
6. **Send and read back.**
   - Send `validate`, show any changeset errors, and stop if there are any. Otherwise send `save`.
   - Record the reply, diff, redirect and any `push_event` frames.
   - Re-read the thread to confirm the reply exists, and store it.
   - If the server rejects the inferred payload, nothing was posted. The tool shows the error, and Michael can retry after a fix.
7. **Fixtures.** The redacted frames become `tests/fixtures/site/reply_*.json` and `notifications.html`. Both of us review them before commit.

Uploads, votes, deletes and edits are **not** part of this capture. Each one gets the same guided capture on its first real use (§2.1).

**3.4.0 status.** `--capture-reply THREAD` is compiled in and hidden from `--help`. It does not create a throwaway HOME, does not copy `session.json`, and does not send `validate` or `save`. Those steps stay stubbed until a reviewed fixture exists. `--dry-run` prints the documented form fields with the LiveView form encoding this client already uses, and it lists every piece that was not invented.

### 4.2 TUI menu categories (required before 3.x is finished)

The terminal menu bar is reorganized into categories. This must ship before 3.x is finished. It is a later minor of its own, and it is not post-3.x polish. The current bar stays until that minor. This note does not implement the menus, does not assign the minor a number, and does not move 3.10.0 or any later numbered stage, including 3.22.0.

Left to right, with Start Scanning pinned on the right:

1. **View** — Nested, Stack, Carousel, and Grid; Site Top; Explore; EB (Executive Board); previous and next post.
2. **Search** — local search and site search. Search history joins this menu: reopen a past query (keyword, nicks, dates, kind, and Where), not a saved list of results. That is its own minor when Search work is touched, and it is not numbered here.
3. **Compose** — New post, Reply, and Quote. Drafts and the outbox join this menu when that stage ships.
4. **Library** — Export, Import, Basket, List, and Maintain.
5. **Bookmarks** — Show, Bookmark current, and Sync with acp.us, as that menu works today. This is its own menu so it stays one click from the bar.
6. **Research** — projects, add, and export, as the Research menu works today.
7. **Sync** — keep the local copy up to date. Today that is site sync and auto-sync. Peer replication of the local database joins this menu later.
8. **Discover** — find people, nodes, and rooms. A stub until the network-node stage (P9). Then peers, public nodes, relays, and the spaces / live badge.
9. **Account** — Session (login and logout) today. Profile, site settings, and password change join this menu in their own stages.
10. **Debug** — verbose, logging, and help.
11. **Start Scanning** — pinned on the right. The scrape options (nicks, latest or trending, mentions, and the rest of today's Scrape menu) stay on this control.

Bookmarks is not an item under Library. Sync and Discover stay two menus. Sync is how this machine's copy stays current. Discover is how you find someone or somewhere else.

Quit is a menu item today and this list does not name it. It stays on the bar until Michael says where it goes.

### 4.3 Hover tooltips (required before 3.x is finished)

Every button and other actionable control in the terminal views, and every menu item, shows a short tooltip on hover. The same applies to the icons on a card and in a detail (the star, the basket, the research ◈, and the rest) and to every entry on the menu bar.

Hover shows a brief explanation of what that control does. When the control has a key, the tooltip includes it, as in `Bookmark (b)`.

One shared helper draws these tooltips. A new control uses that helper, and every control has that text.

This must land before 3.x is finished. It can ship with the menu reorganization (§4.2) or as its own later minor. This note does not implement it, does not assign that minor a number, and does not move 3.10.0 or any later numbered stage, including 3.22.0. The mouse path today handles clicks and the wheel. Hover help is part of this requirement.

### 4.4 Help topics (required before 3.x is finished)

Help stays reachable with `?` and F1. Three changes must land before 3.x is finished. They can ship with the menu reorganization (§4.2), with the hover tooltips (§4.3), or as their own later minor. This note does not implement them, does not assign a number, and does not move 3.10.0 or any later numbered stage, including 3.22.0.

1. **A help icon on the chrome.** A small help icon sits on the interface, and a tooltip may carry the same mark. Choosing it opens Help on the topic for that control.
2. **Topics you can move between.** Help is no longer one flat list of keys. It is sections you step through: Getting started, Browse, Compose, Research, Sync, Discover, Account, Keys, and the others that the app needs. Keys stay inside the Keys topic, and the command-line help stays available from a topic rather than as the whole screen.
3. **Search inside Help.** Typing filters the topics and the key bindings, so a word or a key finds its place.

Today Help is one scroll: the recovery guide, then the key list, then the generated command-line help.

---

## 5. Risks

- **The site's protocol is internal and changes with every deploy.** The hashed bundles, the "An update is available" banner
  and the `toggle-bookmark` → `bookmark` drift all show this. Mitigation: feature probes, bundle-hash gating, fixture
  refreshes and graceful read-only fallback.
- **Account risk:** a malformed frame could post garbage under Michael's name. The preview, validate-first and read-back
  steps, plus starting with replies in a thread Michael chooses, reduce this. Never automate bulk writes.
- **Uploads** may use an external presigned uploader (Backblaze, which is a media host already), so they may be fiddly.
- **Side effects of reads:** view counting. Marking notifications read is accepted (resolved 2026-10-18).

---

## 6. Feeds (client-side; phase P7, 3.17.0 → 3.20.0)

Feeds are named, saved, local views built from rules over the local DB. They can refresh from the
site through targeted syncs, export with the existing formats, and be shared as a portable definition. No
personal data goes into a shared definition.

### 6.1 Rules model
- **Sources:**
  - `nick` (one or more).
  - `search`: a full-text query, reusing the 2.x local search matcher. An FTS5 table `posts_fts` is
    suggested once feeds exist, for speed.
  - `tag`: `#tag`, case-insensitive, from `post_tags`.
- **Each rule** is `include` or `exclude`. Includes combine with `match = "any" | "all"`; excludes always subtract.
  An optional `group` lets you nest one level of `(A OR B) AND C`, which keeps the editor simple.
- **Filters:**
  - `since` / `until`, absolute ISO dates or relative values like `"30d"`.
  - `kinds = ["posts","replies"]`, `min_score`, and `has_media`.
- **Sort:** `newest | oldest | score`.
- Rules compile to one parameterised SQL query, unit-tested rule by rule.

### 6.2 DB
```sql
CREATE TABLE feeds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  description TEXT NOT NULL DEFAULT '',
  definition TEXT NOT NULL,          -- canonical JSON of the rules (same schema as .acpfeed)
  source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local','imported','peer')),
  origin_hash TEXT NOT NULL DEFAULT '', -- blake3 of the imported definition
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_refreshed_at TEXT
);
CREATE TABLE feed_rules (            -- denormalised index for "which feeds use nick X / tag Y"
  feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('nick','search','tag')),
  value TEXT NOT NULL, mode TEXT NOT NULL CHECK (mode IN ('include','exclude')),
  grp INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE feed_seen (feed_id INTEGER, post_id TEXT, seen_at TEXT, PRIMARY KEY (feed_id, post_id)); -- unread counts
```

### 6.3 The definition file (`*.acpfeed`, TOML; JSON accepted)
```toml
format = "acp-feed"
version = 1
name = "Theory reading"
description = "Books, theory and the EB on them"
match = "any"
sort = "newest"
since = "90d"
kinds = ["posts", "replies"]

[[rule]]
kind = "tag"
values = ["books", "theory"]

[[rule]]
kind = "nick"
values = ["hazaldin"]

[[rule]]
kind = "search"
query = "dialectical materialism"

[[rule]]
kind = "tag"
mode = "exclude"
values = ["memes"]
```
- **Validation** uses the same approach as themes:
  - Unknown-key warnings, `version` checked, a 64 KiB cap and a name/charset check.
  - Values are limited (nicks `[A-Za-z0-9_]`, tags without `#`, queries ≤ 500 chars, ≤ 200 rules).
- **Never included:** post ids from bookmarks, research items, the user's own nick (unless typed as a rule),
  notes, local paths, emails, cookies, `last_refreshed_at` and `feed_seen`.
- **Share string:** `acpfeed1:` + base64url(deflate(canonical JSON)), which is copyable (OSC 52) and pasteable.
  Import accepts a file, `-` (stdin) or a share string.
- **Importing** rebuilds the feed from the *recipient's* DB. It then offers a targeted sync for nicks and tags they don't have yet.
  On a name clash the user picks rename, replace or skip.

### 6.4 Refresh
- **Opening a feed** runs it against the local DB, so it's instant.
- **"Refresh from site"** (`R` in the feed view, or `--feed-refresh NAME`) runs targeted, incremental syncs as headless jobs:
  - nick rules → profile scrape (stops at the newest stored post).
  - tag rules → `/rs/tag/{tag}` (`forum/Feed` + `page-next`, incremental).
  - search rules → `/rs/explore?q=…` (`forum/Feed`).
  - Excludes never trigger fetches.
- Optional auto-refresh interval per feed (off by default). Live updates (§2.4) also re-run open feeds.
- With P9, a feed can also pull its matching records from peers.

### 6.5 Site-side tag support (found in the survey)
- Tag pages are `/rs/tag/{tag}` (a `forum/Feed`, with an empty-state message when there are no posts).
- In bodies, tags are TipTap `mention` nodes inserted with `mentionSuggestionChar:"#"` and `label:"#tag"`. People are `mention {id:user_id, label:"@nick"}`.
- The composer autocompletes tags with `tag-query {query}`.
- The site has **no tag-following or tag-subscription feature**. That's something Feeds adds.
- Today we flatten bodies to text, so tags are only recoverable by regex. In the real DB, 48 posts contain `#word`.
  P1 adds `body_json` + `post_tags`, plus a backfill pass through `--refetch-truncated`-style re-reads.

### 6.6 TUI
- **Nick bar:** a "▤ Feeds" section under ★ Bookmarks, showing one row per feed with an unread count. Click or Enter opens the feed view.
- **Feed view:** the normal Body with every view mode. The header chip shows the rules summary, and a "Refresh from site" button.
- **Feed editor** (modal, like the research project editor):
  - Name and description.
  - A rule list where you can add, remove, reorder and toggle include/exclude.
  - Any/all, dates, kinds and sort.
  - A live count of matching posts.
- **Add to feed…** from anywhere: a nick (nick bar or post author), a `#tag` in a body, or the current search.
  It's a modal like Bookmarks/Research, letting you pick a feed or create one. It's in the command palette and context menus.
  The suggested key is `f`, still to be checked against the existing bindings.
- **Actions** (feed list menu and palette): Export…, Share… (shows the string and saves a `.acpfeed`), Import… and Delete.
- **Tombstoned posts (interactions 3.5.0–3.9.0 and feeds 3.17.0–3.20.0).** A post with a tombstone stays in the feed and in the post list. The row shows a tombstone icon, the glyph 🪦, or the ASCII fallback `[RIP]` when the terminal cannot render that glyph. The body stays viewable. Opening it shows the retained content with the same icon and a "deleted" mark. Nothing in these stages drops the body (§7.5).

### 6.7 CLI
| Flag | What it does |
|---|---|
| `--feed-list` | List feeds |
| `--feed-create NAME [--feed-description D]` | Create a feed |
| `--feed-add NAME --nick N \| --tag T \| --search Q [--exclude]` | Add a rule (repeatable) |
| `--feed-remove-rule NAME --nick/--tag/--search` | Remove a rule |
| `--feed-show NAME` | Print the matching posts |
| `--feed-refresh NAME` | Targeted site sync |
| `--feed-export NAME --format json,html [--tar-gz] [-o]` | Export the feed's **contents** (existing exporters) |
| `--feed-share NAME [-o x.acpfeed] [--share-string]` | Export the **definition** |
| `--feed-import FILE\|STRING [--name N] [--replace]` | Import a definition |
| `--feed-delete NAME [--yes]` | Delete a feed |

### 6.8 Tests
- Unit tests:
  - SQL compilation for each rule kind, any/all, excludes and dates.
  - The definition parse/validate round-trip, the share-string round-trip, and rejection of bad input.
  - A test that personal fields are never written.
- Gherkin tests:
  - Create a feed → view it → export JSON/HTML.
  - Share → import into a second throwaway HOME → same results from that DB.
  - Refresh triggers mocked tag, profile and search fetches (fake LV server).
- Version phase: P7, from **3.17.0** (core + view) to 3.18.0 (editor + add-to-feed), 3.19.0 (export) and 3.20.0 (share/import).

---

## 7. Decentralization: nodes and P2P sync on libp2p (phase P9, 3.23.0 →)

Michael's decision: build on **rust-libp2p**. Alternatives were not weighed.

### 7.1 Version and MSRV
- **`libp2p = "0.57.1"`** (released 2026-10-15) declares `rust-version = "1.88.0"`, exactly our MSRV. The sub-crates we
  need also declare 1.88.0:
  - Transports and security: `libp2p-tcp 0.45`, `-quic 0.14`, `-noise 0.47`, `-yamux 0.48`.
  - Protocols: `-identify 0.48`, `-kad 0.49`, `-mdns 0.49`, `-gossipsub 0.50`, `-request-response 0.30`.
  - NAT traversal: `-autonat 0.16`, `-relay 0.22`, `-dcutr 0.15`.
  - Limits and core: `-connection-limits 0.7`, `-allow-block-list 0.7`, `-swarm 0.48`.
- **We're at the edge.** Pin `libp2p = "~0.57.1"` and rely on the MSRV-aware resolver (edition 2024 with `rust-version`).
  Add a CI or pre-commit check, `cargo +1.88 check --features p2p`. A future 0.58 will probably need a newer Rust, which would mean an MSRV bump (open question).
- Feature flags: `tokio, tcp, quic, dns, noise, yamux, identify, ping, kad, mdns, gossipsub, request-response, cbor,
  autonat, relay, dcutr, macros, ed25519`. Everything sits behind a cargo feature `p2p`, so the default build stays slim.
  Release builds turn it on.

### 7.2 Node and identity
- **`--serve`** runs the headless binary as a daemon: it listens, replicates, and optionally keeps scraping (`--serve --sync`).
  - A local control socket (`~/.acp/node/control.sock`, 0600, JSON lines) lets the TUI and CLI talk to a running node.
  - Without `--serve`, the TUI can run an embedded node only while it's open.
- **Identity:** each node has its own libp2p ed25519 `Keypair` (generated on the node, never leaving it) plus an X25519 wrapping key,
  stored in an **encrypted keystore** (§7.13.6), not as a plain `identity.key`.
  - The PeerId is shown in the TUI and printed by `--node-id`.
  - The same key signs records (§7.5).
  - The node is **certified by the user's master key or by an authorizer** (§7.13.2). Its certificate chain travels with its records.
  - `--node-rotate-key` is a re-enrollment (§7.13.3): a new keypair, a new certificate, and the old one revoked.
- **Config:** a `[node]` section in preferences (`listen = ["/ip4/0.0.0.0/udp/4848/quic-v1", "/ip4/0.0.0.0/tcp/4848"]`,
  `network = "<name>"`, limits) and `~/.acp/node/peers.toml`.

### 7.3 Transports and discovery
- **Transports:**
  - **QUIC** (`quic-v1`) is preferred: it builds in TLS 1.3 and multiplexing, and hole-punches better.
  - **TCP + Noise (XX) + Yamux** is the fallback. Add DNS for `/dns4` addresses.
- **Network isolation:** every protocol id is namespaced with the network name, for example `/acp-rs/<network>/kad/1.0.0`.
  - That way we never join the public IPFS DHT. Kademlia runs in server mode only on publicly reachable nodes (AutoNAT decides).
  - For strictly private networks, `pnet` (pre-shared key) is an option, but it only works over TCP. The main
    gate is the application allowlist (§7.4).
- **Discovery:**
  1. Manual peers: `--peer-add /ip4/…/udp/4848/quic-v1/p2p/<PeerId>`.
  2. **Invite codes:** `acpnode1:` + base64url of a signed record `{network, inviter PeerId, multiaddrs, expiry, grant}`.
     Accepting one dials the peer and adds it with the granted trust level.
  3. **Bootstrap peers** listed in config.
  4. **Kademlia** for finding allowlisted peers' current addresses.
  5. **mDNS** on the LAN, **on by default** with a Nearby nodes list and no automatic connection (§7.14.2).
  - `identify` exchanges listen addresses and protocols, and `ping` keeps connections alive.
- **NAT traversal:**
  - **AutoNAT (v2)** detects reachability.
  - Nodes behind NAT reserve a slot on a **relay v2** server. Relaying is on by default in two scopes, local and public (§7.14.3), with limits. The `acp_rs_node_relay` crate also builds a standalone `acp-rs-relay`.
  - **DCUtR** upgrades relayed connections to direct ones by hole punching. Traffic falls back to the relay with a byte cap.

### 7.4 Auth, trust and limits
- **Per-peer trust levels** in `p2p_peers(peer_id, label, trust, added_at, last_seen, addrs, bytes_in, bytes_out, notes)`:
  - `blocked` (see the `allow-block-list` behaviour).
  - `pull-only`: they can fetch from us, and we accept nothing.
  - `exchange`: two-way, and their records are stored as `peer` provenance.
  - `trusted`: as exchange, and we also honour their tombstones and edit observations.
  - `device`: one of **my own** certified nodes (its certificate chains to my pinned master key, §7.13.2). It's the only level whose write intents my publishing node will act on (§7.6).
- **Default:** only allowlisted peers can connect (`allow-block-list` in allow mode). Unknown peers are disconnected after `identify`.
  **Public nodes exist (decided 2026-10-19, §8 Q10).** The default stays a private, invite-only network. `--serve --public` is opt-in.
  - A public node lets strangers dial it and fetch public posts (posts that are visible on the site, and local posts the user has published). Account-private tables, drafts, bookmarks, the keystore, enrollment, and write intents stay closed. An unknown dialer is `pull-only`: it can fetch, and this node stores nothing from it, until the user raises that peer's trust.
  - **Node record.** The signed node record gains `visibility` (`private` or `public`, default `private`). A public record carries the PeerId, the listen addresses, and `visibility`. It does not carry account-private topics or any secret. `p2p_peers` stores the same flag when the user keeps a foreign public node.
  - **Relay scopes (§7.14.3).** The local scope is unchanged. The public relay scope is how other people reach a public node: `--public` also turns on that node's public relay scope, with the same conservative limits (a handful of reservations, short circuits, a daily byte cap). A node may use the public relay scope without publishing its database. A public content node does not relay account-private streams.
  - **Stage.** The flag, the `visibility` field, and the public relay scope land in **3.23.0** with `--serve` and the relay crate. Advertising a public node through Kademlia lands in **3.24.0**. This does not add a version.
- **Connection limits** (`connection-limits`): maximum established connections in and out, a per-peer maximum (2), and a pending-dial maximum.
  On top of that, request-response quotas per peer (requests per minute and bytes per day), plus a global storage cap
  for peer-provided data (default 2 GiB) and a separate media cap.
- **Moderation:**
  - Block a peer: disconnect, add them to the blocklist, and optionally purge or hide everything only they supplied.
  - Hide a nick or post locally.
  - Gossipsub peer scoring and blacklisting of misbehaving peers.

### 7.5 Replication model
- **Records** are immutable, signed and content-addressed:
  - `PostRecord{post_id, thread_id, in_reply_to_id, nick, author_user_id, title, body_json, posted_at, last_edited_at, media:[{url, kind, blob_hash?, size}]}`
  - `Tombstone{post_id, observed_deleted_at}`
  - optional `FeedRecord` / `ResearchRecord` (opt-in).
- **Envelope:** `{record, source: site|peer, observer: PeerId, observed_at, sig}`. The signature is ed25519 by the observer over canonical CBOR.
  `record_hash = blake3(canonical CBOR of the record)`.
- **Keys and dedup:**
  - The logical key is the site `post_id`, and each distinct `record_hash` is a version, stored in `post_versions`.
  - Identical content from many observers is stored once, with several observations.
- **Conflict rules (deterministic, with no vector clocks):**
  1. Records are immutable, so there are never write-write conflicts, only competing versions.
  2. The current version is the one with the highest `last_edited_at`, then the earliest `posted_at`-consistent version,
     then the most observations from site-sourced envelopes, then the lowest hash.
  3. A version **we scraped from the site ourselves always wins** over peer data for the same `last_edited_at`.
  4. **Tombstones (revised 2026-10-19, §8 Q10).** This replaces the earlier working design that dropped the body after a grace period. Never let data go to waste.
     A site-observed deletion and a signed peer tombstone from a `trusted` peer (§7.4) both **keep the post data**. There is no grace-period deletion and the body is not dropped. A tombstone from a peer below `trusted` is shown as "reported deleted by peer X" and does not mark the local copy deleted.
     The post is shown as deleted. The UI uses a tombstone icon (🪦, ASCII fallback `[RIP]`). The content stays viewable and retained locally. Interactions (3.5.0–3.9.0) and feeds (3.17.0–3.20.0) use that icon (§1b, §6.6).
     **Restore.** If the author later publishes again or re-signs the same `post_id`, or retracts the tombstone with a signed restore record, the post id is restored retroactively: the tombstone is cleared and the post displays normally. The restore record is in `docs/PROTOCOL.md`. It is signed by the author's key, it references the tombstone, and a newer timestamp wins. The retained body is what comes back. Replication must not treat a tombstone as a reason to delete bytes.
- **Set reconciliation:**
  - **Range-based set reconciliation (RBSR, Negentropy-style)** over items sorted by `(posted_at, record_hash)`.
  - Peers exchange fingerprints (an XOR/sum of hashes plus a count) for ranges and split them recursively until differences are found,
    so very little data moves when sets are nearly equal and there's no per-peer state.
  - It runs over a **request-response** protocol `/acp-rs/<net>/sync/1` (CBOR codec, size caps, timeouts),
    scoped by filter (all, nicks, threads or a feed definition).
  - A per-peer high-water mark (`p2p_sync_state`) makes periodic syncs cheap.
- **Announcements:** **gossipsub** topic `acp-rs/<net>/announce/v1`.
  - Messages are small and signed (`ValidationMode::Strict`, message-id = blake3), carrying `{record_hash, post_id, nick, posted_at}`.
  - Receivers fetch what they lack through sync or a direct record fetch. A per-topic size cap and peer scoring apply.
- **Blobs (media):**
  - Opt-in. `/acp-rs/<net>/blob/1` is request-response by blake3 hash, in 1 MiB chunks, resumable, verified by hash, under the media quota.
  - Signed-URL media is canonicalised as in 1.26.1, so the same file dedups across peers.
- **Provenance in the UI:** a marker on posts that only peers have supplied ("relayed by <label>"). When the site later serves the same
  post, it's marked "confirmed".

### 7.6 Writing to acp.us through nodes (resolved 2026-10-18)

Michael wants nodes to sync **to** the site as well as from it. The governing rule: **a write is only ever published to acp.us
with the authoring user's own session, on a node that user controls.** No node ever holds, receives or uses another user's
cookies or tokens. Peers only carry signed *intents*.

- **Write intents.** Every site write (§2.1 `SiteAction`) can be wrapped as an immutable, signed record:
  `WriteIntent{intent_id (ULID), account_user_id, nick, action, target (thread/post id), body_json, attachments:[blob_hash],
  preview_hash, confirmed_at, created_at, expires_at (default 24 h), hop_limit}`.
  - It's signed by the authoring device's node key.
  - The author confirms on the authoring device with the normal preview, so that's where the human confirmation happens.
  - `preview_hash` binds the confirmation to exactly what will be posted.
- **Who publishes:** only a **publisher node**, which is a node holding that account's own `session.json` with `node.publisher = true`. It may be the same machine as the author.
  - **Case 1, own node:** the author's device holds the session itself, so it publishes directly through its outbox (§2.2). This is the default and the simplest case.
  - **Case 2, linked devices:** a device without a session (for example a laptop that doesn't keep cookies, or a future phone) sends the intent
    to the same user's publisher (for example a home `--serve` box). This only works between peers linked as `device`.
    Linking is the enrollment in §7.13.3: the device becomes a certified node with the `intent-sender` capability, with a confirmation on both ends.
- **Checks on the publisher before sending:**
  - The signature is valid, and the signer is a linked `device` (or the publisher itself).
  - `account_user_id` equals the session's `current_user.id`, re-read from the site.
  - The intent hasn't expired, `hop_limit` > 0, and the intent hasn't been seen before.
  - Writes are enabled on the publisher, and rate limits allow it. Feature and bundle-hash probes pass.
  - The `preview_hash` matches the payload it rebuilds.
  - Optional: destructive actions (delete, username change) also need a confirmation on the publisher (sub-question).
  - Anything else is dropped and logged. An intent for another account is **never** published, whatever its signature says.
- **Transport between devices:**
  - Directly over libp2p (request-response `/acp-rs/<net>/intent/1`), or through a **relay v2 circuit**.
    The relay carries the end-to-end Noise/QUIC-TLS stream and can't read it.
  - Third-party peers never see intent plaintext. Store-and-forward through other peers while the publisher is offline is a sub-question.
    If it's adopted, the intent is sealed to the publisher's key, opaque to forwarders, and bound by `expires_at`.
  - Otherwise the intent waits in the author's own outbox until the publisher is reachable.
- **Dedup (no double-posting):**
  - `intent_id` is the idempotency key. The publisher keeps
    `p2p_intents(intent_id PRIMARY KEY, account_user_id, status ∈ received|publishing|published|rejected|expired, site_post_id, publisher, updated_at)`.
    A repeated delivery gets the stored result back.
  - `publishing` is crash-safe, as in the outbox: before any retry, the publisher reads the thread back and looks for a post by
    this account whose body hash matches `body_json` within the intent's time window.
  - With several publishers for one account, only one acts. That's the lowest PeerId among the account's online publishers, plus a short
    claim record exchanged between them. The read-back check is the final guard.
  - State-type actions (vote, bookmark, subscribe) are idempotent by design: the intent carries the target state, not a toggle.
- **Loop prevention:**
  - Only `WriteIntent` records can ever cause a site write, and only on a publisher for that account.
  - `PostRecord`s, tombstones and receipts are observations. They **never** turn into writes.
  - Intents are never derived from site data. They carry a `hop_limit` and an expiry, and gossipsub dedups by message-id.
  - Our own scraping never re-publishes anything.
- **Results flowing back into the network:**
  - After publishing, the publisher reads back the new post and emits a normal site-observed `PostRecord` (`source = site`, signed by
    the publisher as observer), plus a signed `PublishReceipt{intent_id, site_post_id, published_at, publisher}`.
  - Both replicate through gossip and reconciliation like any other record. The authoring device marks its outbox item `sent`.
  - Other nodes simply see a new site post, with provenance "site, observed by <publisher>".
  - Unpublished intents are **not** replicated as posts, so peers never see posts that never reached the site. The author sees them locally as "pending".
- **What intents never contain:** cookies, CSRF or LiveView tokens, emails, or anything from §7.7.
- **Revocation:** `--device-revoke PEER` removes a device. The publisher then rejects that key's intents, including ones already queued.

### 7.7 Never shared
- `session.json`, cookies, CSRF and LiveView tokens, the login email, preferences and logs. A session never leaves the machine it was created on.
- The keystore, node and master secrets, data keys (only their wrapped forms move, and only to your own certified nodes), certificates' private parts, and backups (§7.13).
- The outbox and drafts, except intents sent to my own linked publisher (§7.6), which never reach third-party peers in readable form.
- Notifications, votes and subscriptions. Also bookmarks, research projects and feeds, **unless opted in per item**.
- Local file paths.
- The serializer uses an explicit allowlist of record types, with tests that fail if any other table leaks.

### 7.8 Coexistence with site sync
- The site stays the source of truth, and sync goes both ways:
  - **Site → network:** any node that scrapes emits site-observed records, which gossip and reconcile to peers. A node with `--serve --sync`
    can share scraping duty, so not every member has to scrape the site.
  - **Network → site:** only through §7.6 intents, published by the author's own publisher node with the author's own session.
    Peer *data* (`PostRecord`s) never triggers a site write.
- Scraping, P2P and publishing all write to the same tables, with `source`. Site-observed versions win conflicts (§7.5).
- `--sync` can do "site then peers". A publisher also drains pending intents once it's online.

### 7.9 CLI / TUI
- **CLI:**
  - Node: `--serve [--relay] [--public]`, `--node-id`, `--node-status`.
  - Peers: `--peer-add ADDR [--trust T]`, `--peer-list`, `--peer-trust PEER T`, `--peer-block PEER [--purge]`, `--peer-remove PEER`.
  - Invites: `--invite-create [--trust T] [--expires 7d]`, `--invite-accept CODE`.
  - Syncing: `--p2p-sync [PEER] [--feed NAME]`, `--p2p-share feed|research NAME`.
  - Publishing (§7.6): `--publisher on|off`, `--device-link` (creates a `device` invite), `--device-list`, `--device-revoke PEER`,
    `--intents-list`, `--intent-cancel ID`.
- **TUI:** a **Nodes** section (menu and palette; suggested key F7) with these tabs:
  - **This node:** PeerId, listen addresses, NAT status and relay.
  - **Peers:** trust, last seen, bytes and actions.
  - **Invites.**
  - **Devices & publishing:** linked devices, whether this node is a publisher, and pending, published and rejected intents with their reasons.
  - **Sync activity.**
  The header shows a node chip (`◉ 3 peers`) when the node is running.

### 7.10 Testing
- **In-process multi-node tests:** two to five swarms over libp2p's `MemoryTransport`, using `libp2p-swarm-test` helpers.
  They cover reconciliation convergence, edits, tombstones and trust levels, blocking, quotas, and that private tables never leak.
- **Property tests** (proptest) for RBSR: random set differences always converge, and the fingerprint maths holds.
- **Gherkin:**
  - Invite → accept → sync between two throwaway HOMEs.
  - Block a peer → purge.
  - Feed sharing over P2P.
  - **Write relay against the fake LV server:**
    - device → relay → publisher publishes exactly once, even when the intent is delivered twice or two publishers are online;
    - the receipt and the site record don't re-trigger a write (loop test);
    - an intent for another account, an expired intent, a bad signature, a revoked device or a mismatched `preview_hash` is rejected;
    - the relay node never sees plaintext.
- No test touches the real site or the public internet. The node binds `127.0.0.1` / memory only in tests.

### 7.11 Versions
| Version | Contents |
|---|---|
| **3.0.0** | Identity, encryption at rest, backup and restore (§7.13 without a node): master secret, BIP-39 phrase, SLIP-39 3-of-5 shares, pinned `acp-id`, SQLCipher, the local at-rest key and the account data key, `--encrypt-db`, `*.acpbackup`. Breaking for a 2.x plaintext database. Not in this release: authorizer and node certificates, enrollment, revocation, relays, peers. |
| **3.1.0** | Job details in the terminal UI. Click the status bar or the header running mark. The window shows state, elapsed time and the log, and it does not show the password or the database key. |
| **3.2.0** | `acp_rs_node_core`. Identity crypto, canonical CBOR, and the secret-store interface move out of the terminal crate. `docs/PROTOCOL.md` starts with the CBOR rules and test vectors. CLI flags, the encrypted database, and `.acpbackup` format version 1 do not change. |
| **3.3.0** | Local node identity. An independent ed25519 and X25519 keypair, sealed in a keystore that backups skip, certified by the master on the first machine. `--node-init`, `--node-id`, and the header chip. Not in this release: `--serve`, mDNS, relays, enrollment over the network, peers, replication. |
| **3.22.0** | Password change / re-key (§7.13.13). Local data only: the SQLCipher database key, the sealed keystore, the sealed phrase or entropy, and the backup-wrapping key, re-wrapped atomically with rollback. The old password is required. The master, the `acp-id`, and node certificates stay as they are. Tests: the data stays readable, and the `acp-id` is unchanged. Before the daemon and the OS keyring. |
| **3.23.0** | `--serve`, enrollment over the network, renewal, revocation gossip (§7.13; the master and the local certificate format already shipped), pairing code and `--join`, mDNS on by default, relay crate and `acp-rs-relay`, peers and peer management, invites, allowlist, node roles (opt-in), cross-platform service install, post replication (sync + gossip) and provenance UI (§7.14). Also in this stage: public nodes (`--serve --public`, §7.4) and OS keyring unlock for the daemon behind `SecretStore` (§7.13.6, §7.14.5). Repository publishing waits (§7.14.11). |
| 3.24.0 | NAT traversal (AutoNAT, relay, DCUtR) and Kademlia, including discovery of public nodes |
| 3.25.0 | Write relay to the site: intents, certified devices, publisher, receipts (§7.6) |
| 3.26.0 | Media blobs |
| 3.27.0 | Opt-in feed and research sharing |
| 3.28.0 | Draft review: links, comments, suggestions (§7.14.8) |
| 3.29.0 | `acp_rs_node_ffi` and the first Dart SDK release (§7.14.7) |

### 7.12 Privacy and security risks
- A node reveals its IP to peers, and gossip reveals who has which posts.
- Scraped content from a members-only forum is redistributed between nodes. The Party allows this (§8 Q1, resolved 2026-10-18). The default network is private and invite-only. Public nodes are an opt-in (§7.4, decided 2026-10-19).
- Malicious peers could forge "observed" posts. Signatures only prove who relayed a record, not that the site said it.
  That's why the UI marks peer-only provenance and site confirmation wins.
- There's a resource-exhaustion risk, handled by the limits and quotas.
- **A stolen node key could publish as its user** through that user's publisher. Mitigations (§7.13): node certificates last 365 days and are renewed only by an authorizer,
  a revocation spreads by gossip and by a revocation-first handshake, intents expire, rate limits apply, the publisher logs which node sent what, and destructive actions can optionally be confirmed on the publisher too.
  A revoked node keeps whatever it already read, which is why a key epoch rotation follows a revocation.
- A stolen authorizer can enroll rogue nodes. Its limits, the `NodeEnrolled` notices on every node and the master's batch re-certification (§7.13.5) bound the damage.
- A publisher node holds a live session around the clock. It's protected only by file permissions (0600), like today's `session.json`.

### 7.13 Keys, identity, encryption at rest, backup and recovery (decided 2026-10-18)

Michael reviewed and agreed to the design below in discussion on 2026-10-18. Items still open are listed in §8 Q11.
It replaces the single per-node identity key of the first draft of §7.2.

**Summary of decisions**
1. A user is identified by a **master key**, kept offline. Nodes have **their own independent keypairs** and are *certified* by the master, or by an
   **authorizer** the master has certified. Node keys are never derived from the master.
2. The master is created from a random seed shown as a **24-word recovery phrase**, and a **mandatory password** is part of it. The email address
   is never an input to any key.
3. The recovery seed is split with **Shamir secret sharing (SLIP-39 style), 3 of 5**.
4. The master key sits in a safe, and day-to-day operation works without it, through the **authorizer certificate**.
5. The **whole local database is encrypted at rest**. Data keys use **envelope encryption**: every data key is wrapped to every authorized node and to the master.
6. Replication sends **plaintext records inside the encrypted libp2p link** (Noise or QUIC-TLS), and the site sync uses HTTPS.
7. There is a full **backup and restore** (§7.13.7), so losing a machine, a key or the whole HOME is recoverable.
8. Identity is **pseudonymous**. No legal name is part of the protocol. A link to a Red Star nick is optional and proven by a signed claim.

#### 7.13.1 Key hierarchy

| Key | Where it lives | Lifetime | Purpose |
|---|---|---|---|
| **Master** (ed25519 signing + X25519 wrapping, both derived from one master secret with HKDF labels) | Nowhere on disk. Recreated in memory from phrase + password (or 3 shares + password) during a ceremony, then zeroized. | Permanent until rotated | Root of trust. Signs authorizer certs, node certs (when no authorizer is used) and revocations. Can unwrap every data key. |
| **Authorizer** (an ordinary node's key plus an authorizer cert) | On one trusted device, in its encrypted keystore | 30 days, renewed by the master | Adds, renews and revokes nodes day to day, and re-wraps data keys for new nodes. |
| **Node** (ed25519 signing, which is also the libp2p identity, plus X25519 wrapping) | On that node only, in its encrypted keystore. Generated on the node, and the secret never leaves it. | Cert 365 days, auto-renewed | Signs records and intents, authenticates to peers, and unwraps data keys. |
| **Data key (DEK)** | Wrapped copies only (§7.13.6) | One per key epoch | Encrypts the local database. |

- **User id (`acp-id`):** the blake3 hash of the master public key, shown in base32 with a checksum. Nodes *pin* it when they join an account.
- **Master creation:**
  - 256 random bits become the recovery phrase (24 BIP-39 words).
  - `master_secret = HKDF(seed ‖ Argon2id(password, salt = blake3("acp-master" ‖ seed)))`, so the salt comes from the random seed, never from the email.
  - The signing and wrapping keys are derived from `master_secret` with distinct HKDF labels.
- **The password is mandatory.** There is no passwordless mode, and no password hint or reset.
  - Minimum length 8 plus a strength check (zxcvbn-style).
  - Argon2id parameters are stored in the file headers, and the default is tuned to about 1 s on the user's machine with a floor of 64 MiB.
  - The phrase alone, or shares alone, can never reproduce the master.
- **A wrong password still derives *a* master** (as with any seed-plus-passphrase scheme), so the tool always shows the `acp-id` and refuses to continue unless it
  matches the one in the backup header or the one already pinned by known nodes.
- **No email or real name goes into any key, certificate or record.** Optional claims (§7.13.9) are separate, signed and opt-in.

#### 7.13.2 Certificates

All certificates and revocations are canonical CBOR, signed with ed25519 and versioned.

- **Authorizer cert:** `{type:"authorizer", account: master_pub, subject: authorizer node pubkey, serial, issued_at, expires_at (default +30 d),
  limits{max_nodes, max_node_ttl, history_grant: "allowed"|"forbidden", may_revoke: "own"}}`, signed by the master.
- **Node cert:** `{type:"node", account, node_sign_pub, node_enc_pub, peer_id, label, serial, issuer (master or authorizer pubkey),
  issued_at, expires_at (default +365 d), caps[sync, publish, intent-sender]}`, signed by the issuer.
  The authorizer signs only while its own certificate is still valid. The node certificate is not shortened to that authorizer expiry, so a 30-day authorizer can issue a 365-day node certificate.
- **Lifetimes (decided 2026-10-19, §8 Q11).** An authorizer certificate lasts 30 days. A node certificate lasts 365 days. Verifiers allow 5 minutes of clock skew. Renewal, the 7-day grace period, and revocation are still to be built, in the stages already planned for them (3.23.0). 3.3.2 compiles these lifetimes. 3.3.0 and 3.3.1 had them reversed. A node from those versions keeps the short certificate until `~/.acp/node` is removed and `--node-init` is run again, which creates a new node key.
- **First machine (accepted 2026-10-19).** `history_grant` is `allowed` and `max_nodes` is 16, because this machine already holds the database. `max_node_ttl` is the node lifetime, 365 days. A later enrollment still asks the approver to choose history access (none, since a date, or full), as in §7.13.3.
- **Chain:** `node cert → authorizer cert → master pubkey`, or `node cert → master pubkey` when the master signed it directly.
  The chain travels with the node's records (§7.5 envelopes carry the chain or its hash, with the chain fetchable on demand).
- **What a verifier checks:**
  - every signature, and the chain ends at the pinned master key
  - the cert was valid at the time the record was *first seen locally* (not the time the signer claims)
  - neither cert is in the account's revocation list for that time
  - the capability needed for this action, for example `intent-sender` for a write intent
- **Who trusts whom:** peers learn an account's master pubkey from an invite or enrollment (`device` trust, §7.4). It is pinned, so a certificate chain only counts
  for an account that was explicitly pinned. Other users' accounts are recognised for provenance only.

#### 7.13.3 How certificates reach a new node (enrollment)

1. **Node generates its keys.** `--node-init` creates the node signing and wrapping keypairs locally and asks for a local unlock passphrase. It then builds an
   **enrollment request** `{node_sign_pub, node_enc_pub, proposed label, proposed caps, nonce}` and shows a short **verification code**
   (6 words derived from the request hash).
2. **Request reaches the authorizer** by one of two routes:
   - **Out of band:** `--enroll-request` prints an `acpenroll1:` string (also as a QR code), which the user copies or scans to the authorizer.
   - **Over libp2p**, when the new node can already dial the authorizer: the request travels over the Noise stream of a `device` invite (§7.4) and appears in the authorizer's
     "pending enrollments" list.
3. **The user approves on the authorizer.** `--enroll-approve STRING` (or the TUI Nodes › Devices pane) shows the label, the node key fingerprint and the verification code. The user:
   - compares the code with the one on the new node's screen,
   - chooses **history access** (none, since a date, or full), and the capabilities,
   - enters the password to unlock the authorizer's keystore.
   The authorizer enforces its limits (`max_nodes`, `max_node_ttl`, `history_grant`).
4. **The authorizer issues the bundle.** It signs the node cert and returns an **enrollment bundle**:
   - the node cert and the authorizer cert (the chain) and the master public key and `acp-id`
   - the **data-key epochs wrapped to the new node's X25519 key** (HPKE), limited to the chosen history access
   - a snapshot of the account's **revocation list** and the current epoch number
   - bootstrap peer addresses for the account
   It returns as an `acpenroll1-reply:` string or over the libp2p stream. The bundle holds **no usable secret except wrapped keys that only the new node can open**, so
   a copy of it is safe to pass over an untrusted channel. It is bound to the request nonce and the node key, so it can't be replayed for another node.
5. **The new node verifies and joins.** It checks the chain against the **`acp-id` the user confirmed** (typed or scanned from the authorizer's screen, never trust on first use).
   It stores the cert in its encrypted keystore, unwraps the keys, connects to peers, and starts syncing the encrypted data. It announces itself with its cert chain.
6. **Everyone is told.** The authorizer publishes a signed `NodeEnrolled{label, node_pub, serial, issuer}` on the account-private topic, and **every node of the account shows
   "New node 'laptop' enrolled by 'home'"** with a one-key revoke action. An enrollment the user didn't make is visible within minutes.

**First setup (no authorizer yet):** `--identity-init` is a one-time ceremony on the first machine. It generates the phrase, asks for the mandatory password,
prints the `acp-id` and the phrase (never logged, never copied to the clipboard), and walks through splitting it into 5 shares (§7.13.8).
It then signs the **authorizer cert for that machine** and its first data-key epoch, and **erases the master from memory**. The master never touches the disk.
From then on the master is needed only for the cases in §7.13.4 and §7.13.5. The same ceremony (`--master-sign`) runs on any machine, from the phrase and password or from 3 shares and the password.

#### 7.13.4 Refreshing certificate expiry

- **Node certs (365 days):**
  - A node starts renewing when a third of its lifetime is left, and retries with backoff.
  - It sends a `RenewRequest{serial, node_pub}` signed by its **current, still-valid** key over `/acp-rs/<net>/cert/1` to any reachable authorizer for the account.
  - The authorizer checks that the node isn't revoked and that the policy still allows it. It can sign only while its own certificate is valid.
    It issues a new cert with the **same keys**, a **new serial**, and a new 365-day expiry. Nothing needs the user, and each renewal is logged.
  - A **changed key is never renewed this way.** It goes through enrollment again.
  - A node whose cert expires before it reaches an authorizer enters a **grace period** (default 7 days): it keeps syncing read-only, new intents are refused, and a banner explains why.
    After that, peers disconnect it until it renews. A node that was offline for weeks just renews on its next contact.
- **Authorizer cert (30 days):**
  - It needs the master. `--authorizer-renew` is a short ceremony: bring out the phrase (or 3 shares) and password, the master signs a new authorizer cert with a new serial, and the master is erased again.
  - The 60-, 30- and 7-day reminder list was drafted for a year-long authorizer certificate. When `--authorizer-renew` is built, the reminders are timed for a 30-day certificate (banner, `--node-status`, and `--serve` logs).
  - If it lapses, the authorizer can't add or renew nodes. Existing node certs stay valid until their own expiry (365 days). Renewing the authorizer later restores enrollment. Nothing already issued is lost.
  - Renewal, the 7-day grace period, and revocation are not built yet. They stay in 3.23.0.
- **Clocks:** certificate times are checked with ±5 minutes of tolerance. A record counts as validly signed if the cert was valid when the record was first seen locally.
- **The master has no expiry.** Rotation is a separate, rare procedure (`--master-rotate`): the old master signs a "moved to" record for the new one, new shares and a new phrase are made, and every node is re-certified.

#### 7.13.5 Revocation

- **The record:** `Revocation{account, seq, target: node_pub | authorizer_pub | serial, effective_from (T), reason, issuer, sig}`.
  - **Who may issue:** the master may revoke anything. An authorizer may revoke only nodes it issued. A node can't revoke others.
  - **T** is the earliest time the key might have been misused. It defaults to now, and `--since TIME` backdates it for a suspected compromise.
  - Revocations form an **append-only, hash-chained list per account**, so a missing item is detectable. Every peer keeps it forever.
- **Spreading it:**
  - It's published on the account-private gossip topic `acp-rs/<net>/account/<acp-id>/revoke/v1`.
  - A **revocation-first handshake:** on every new connection, peers exchange the head (sequence number and hash) of the revocation list for the accounts they track, and the side that is behind
    fetches the missing revocations **before any records are synced**. A node that was offline learns of a revocation the first time it talks to anyone who knows it.
  - **Safety net:** if a revocation never reaches a peer, the exposure ends when the node cert expires (at most 365 days) and renewal is refused.
- **What happens to the revoked node:**
  - Every node that learns of it **disconnects and blocks the PeerId**, refuses its intents and renewals, and **stops wrapping data keys to it**.
  - If the revoked node is honest, it sees its own revocation, stops syncing, marks itself revoked and offers to wipe its data.
  - **It keeps what it already has:** its stored data and any key epochs it held. A hostile node can't be wiped remotely, and the tool never promises that.
- **What happens to everything it signed:**
  - **Records signed before T** stay valid.
  - **Records after T are quarantined:** kept, but flagged "signed by a revoked node", left out of conflict resolution (§7.5) and not counted as an observation, unless the site or another unrevoked observer confirms them.
  - Because a compromised node can backdate `observed_at`, the comparison with T uses **the time the envelope first reached this node**, not the claimed time.
  - **Intents:** queued intents from it are cancelled and never published. Anything already on the site stays there, and the user removes it with a normal delete.
  - `--quarantine-list` and `--quarantine-accept ID` let the user review what was quarantined.
- **What happens to every node that talked to it:**
  1. It verifies the signature and the chain, appends to its local revocation list and stores it.
  2. It drops the connection and blocks the PeerId, and removes `device` trust if the node had it.
  3. It quarantines records from that observer after T (above), and shows a summary ("12 records from 'laptop' after T are quarantined").
  4. It forwards the revocation to its other peers (gossip, plus the handshake).
  5. Peers that only *relayed* the revoked node's records are not touched. Their own signatures are valid, records are content-addressed, and a relay doesn't vouch for the content.
  - Other users' nodes follow the same steps for the account, if they track it. If they don't, they learn it the next time they see a record from that account.
- **Key epoch rotation:** revoking a node that held keys starts a new epoch, E+1.
  - The authorizer or master creates a new data key, wraps it to every remaining node and to the master, and sends it as a signed `KeyEpoch` on the account-private topic.
  - New and changed private data is written under E+1. Older rows are re-encrypted lazily in the background, and `--rekey` forces it at once.
  - Be honest in the UI: this stops the revoked node reading **new** data, but it can't take back what it already holds.
- **Revoking an authorizer (master only):**
  - Every certificate it issued is invalid from T. The master can re-certify the good nodes directly in one batch (`--authorizer-replace` lists the current nodes and signs them), or name a new authorizer.
  - Nodes enrolled after T must be reviewed (the `NodeEnrolled` notices make this list), and are confirmed or revoked.
  - If the authorizer was compromised, T is set to the suspected time and a new key epoch is started.
- **A lost or stolen node:** `--node-revoke PEER [--since T] [--reason TEXT]` on the authorizer. **A lost master** is recovered with the phrase and password or 3 shares and the password (§7.13.8).

#### 7.13.6 Encryption at rest and envelope encryption

- **Scope:** the entire local database is encrypted. So are the keystore, `session.json` (cookies move into the keystore), the outbox and drafts, and all preferences that hold secrets.
- **Implementation candidate:** SQLCipher through `rusqlite` (`bundled-sqlcipher`). It encrypts at page level, so the existing queries and FTS5 search keep working unchanged.
  The alternative is encrypting only the private tables column by column, which is simpler but leaves the cached posts readable. 3.0.0 uses SQLCipher for the whole database (§8 Q11).
- **Envelope scheme:**
  - A random 256-bit **data key (DEK)** per epoch encrypts the database.
  - Each DEK is stored **wrapped** (HPKE, X25519) to **every authorized node's wrapping key and to the master's wrapping key**.
    The master can therefore open every epoch on its own, and **data is never encrypted to a child key alone**.
  - A new node receives wraps for the epochs it's allowed (§7.13.3). A restore from shares uses the master's wrap (§7.13.7).
  - Each node also holds its own copy wrapped by the **local unlock passphrase** (Argon2id), so a stolen disk is useless without it.
- **Unlock:** the passphrase is requested at TUI start. It is never stored in plain text.
  **Daemon unlock (decided 2026-10-19, §8 Q11).** A future daemon may read the unlock secret from the OS keyring when the user opts in, and it may also start locked and wait for an unlock. The keyring sits behind the `SecretStore` interface from 3.2.0: Secret Service on Linux, Keychain on macOS, and Credential Manager on Windows. It lands with `--serve` and the service install in 3.23.0. It is not built in 3.3.1.
- **Table `key_epochs(epoch, created_at, reason, issuer, wraps BLOB)`** keeps this metadata. Secrets are held in zeroizing buffers and excluded from logs and core dumps.
- **Between nodes and the site:** records travel as plaintext inside the Noise or QUIC-TLS link (§7.3), and site sync uses HTTPS. Private tables only sync between the user's own certified nodes.
- **Migration:** `--encrypt-db` makes a backup (§7.13.7), converts the database, verifies row counts and hashes, then replaces it. The old file is removed only after a successful verification.

#### 7.13.7 Backup and restore

- **`--backup [-o FILE]`** writes one `*.acpbackup` archive:
  - A consistent snapshot through SQLite's online backup API, plus preferences, themes, feeds, research, outbox and drafts, `peers.toml`, the node certificate chain, the revocation list and `key_epochs` with their wraps.
  - Packed as tar, compressed with zstd, and **encrypted with XChaCha20-Poly1305** under a key from the password (Argon2id, random salt in the header).
  - **Header:** format version, schema version, app version, created_at, `acp-id`, epoch list and a blake3 checksum of the contents.
  - **Never included:** `session.json` (sign in again with an email code) and the **node's secret key** (a restored machine is enrolled as a new node, so one backup can't clone a node identity).
- **`--restore FILE`:**
  1. Verifies the checksum, the format version (refuses a newer one) and the password, and checks the header's `acp-id`.
  2. **Same machine, node key intact:** restores the database and settings only.
  3. **New machine, or node key lost:** asks for the phrase and password, or 3 shares and the password. It recomputes the master and checks the `acp-id`.
     Using the master's wrap it unwraps every data key, creates a fresh node keypair, and enrolls it as a new node with full history through the master (or any reachable authorizer).
  4. Runs schema migrations forward and verifies counts.
- **Scheduling:** `--backup-schedule daily|weekly --keep N --dest DIR`, written by the headless child, with retention and a verify step.
- **Peers help but don't replace backups:** public posts can be re-synced from any peer, but the private tables and the keys can't.
- **Tests (Gherkin plus unit):** backup then wipe then restore gives identical row counts and hashes. Wrong password, tampered archive, truncated file, newer format and wrong-phrase restore (different `acp-id`) are all refused. A restore into a fresh HOME works from phrase and from 3 shares.

#### 7.13.8 Recovery phrase and shares

- **Phrase:** 24 BIP-39 words encoding the 256-bit seed. It is shown only in a full-screen prompt at creation, never logged, and never copied to the clipboard.
- **Shares:** the seed is split with **SLIP-39 Shamir secret sharing, 3 of 5** (a single group by default), and `--shares-print` produces a printable sheet for each holder.
  - Each share carries the `acp-id`, a set id and the share index, and has a built-in checksum.
  - The **password is not in the shares.** Colluding holders get the seed at most, and the seed alone can't produce the master. The password stays in the owner's head (or their own password manager).
  - `--shares-verify` checks any 3 shares without writing anything down, and the tool reminds you to run it after setup and once a year.
  - Reissuing shares of the *same* seed can't invalidate old ones. Truly invalidating a share set means rotating the master (`--master-rotate`), and the UI says so.
- **Crates:** use vetted implementations (`bip39`, a SLIP-39 crate), pass the official SLIP-39 test vectors, and ask for an external crypto review before network enrollment in 3.23.0. 3.0.0 shipped the phrase, the shares and encryption at rest ahead of that review. 3.3.0 ships the local certificate format ahead of it as well. The format is in `docs/PROTOCOL.md` and is covered by test vectors.
- **Warnings shown at creation:** no password reset exists, and a lost password together with lost shares means the data can't be recovered.

#### 7.13.9 Identity claims (optional, never required)

- Pseudonymous by default. The protocol carries no legal name.
- A user may publish a signed `Claim{acp-id, nick, issued_at}` and prove it by placing the same claim in their Red Star bio. That links the `acp-id` to the nick publicly, so it is opt-in and revocable.
- A real-name attestation, if anyone wants one, is a separate signed claim by the user and is never required by any feature.

#### 7.13.10 How this changes the rest of §7

- **§7.2:** the node key is a node-level key, certified as above. `--node-rotate-key` becomes a re-enrollment. The PeerId is still derived from the node key.
- **§7.4:** the `device` trust level means "a node whose certificate chains to my pinned master".
- **§7.5:** envelopes carry the cert chain, and verification follows §7.13.2.
- **§7.6:** a *linked device* is a certified node with the `intent-sender` capability, and a *publisher* is a node with the `publish` capability. Linking uses the enrollment in §7.13.3 instead of a separate invite.
- **§7.7:** keystore, certs, wraps and backups are never shared. The serializer allowlist gets tests for them.
- **§2.3 (DB):** new tables `identity_certs`, `revocations`, `key_epochs`, `quarantine`, `backups`, plus the `p2p_*` tables.

#### 7.13.11 CLI summary

`--identity-init`, `--acp-id`, `--node-init`, `--enroll-request`, `--enroll-approve`, `--enroll-list`, `--node-list`, `--node-revoke`, `--authorizer-renew`, `--authorizer-replace`,
`--master-sign`, `--master-rotate`, `--shares-print`, `--shares-verify`, `--rekey`, `--encrypt-db`, `--backup`, `--restore`, `--backup-schedule`, `--quarantine-list`, `--quarantine-accept`.
The TUI Nodes section (§7.9) gets an **Identity** tab (acp-id, certificate expiry, key epochs, shares reminder) and a **Backups** tab.

#### 7.13.12 Testing

- Unit tests for the key derivation (known-answer vectors), the certificate chain and expiry checks, wrap and unwrap, and each redaction of keys from logs.
- In-process multi-node tests (§7.10): enrollment end to end, renewal, expiry and the grace period, revocation spread (including a node that was offline), quarantine, epoch rotation (a revoked node can't open the new epoch), authorizer replacement, and a forged or replayed enrollment bundle.
- SLIP-39 and BIP-39 official test vectors. A 3-of-5 recovery test with every combination of three shares, and a test that two shares fail.
- Backup and restore round trips (§7.13.7).
- No test touches the real site or the public internet.

#### 7.13.13 Password change / re-key (3.22.0)

**Decided 2026-10-19 (§8 Q11).** A password change does not change the master and does not change the `acp-id`. Node certificates stay valid. The password is not an input that rotates the account.

`--change-password` asks for the old password and then re-wraps only local data, in one transaction. If any step fails, the whole change rolls back and the old password still opens everything it opened before:

- the SQLCipher database key
- the sealed node keystore
- the sealed phrase or entropy
- the backup-wrapping key

3.0.0 already ships a `--change-password` that keeps the `acp-id` and rewraps the database bootstrap, and reseals the phrase when that phrase used the same password. 3.22.0 is the release that makes the four wraps above one atomic re-key, including the keystore and the backup-wrapping key. Archives already written stay sealed with the password that wrote them. This stage is before the daemon and the OS keyring (3.23.0).

**Tests.** After a successful change, the database, the keystore, and the phrase open with the new password and the stored posts are readable. The pinned `acp-id` is byte-for-byte the same. A forced failure halfway through leaves the old password working and the `acp-id` unchanged. A wrong old password changes nothing.

### 7.14 Onboarding, operations, crates, local-first posts and review (decided 2026-10-18)

These decisions come from the second review with Michael on 2026-10-18 (scenarios: first install with many devices, a libp2p-only Dart app, and a restore with only backups).
They extend §7.13. Where this section and an earlier section disagree, **this section wins**.

#### 7.14.1 Decentralization is optional (on by default)

- First run shows a checkbox, **"Join the decentralized network"**, **checked by default**. It can be unchecked, and changed later in settings. It is also a build feature (`p2p`) and a runtime setting (`[node] enabled`).
- **Unchecked:** no master key, phrase, shares, certificates or pairing. The app stays a site client. The local password and encryption at rest still apply, and backup/restore needs only the password. No recovery phrase is offered to these users.
- **Turned on later:** runs the same ceremony as a fresh install (`--identity-init`), and the machine becomes the first authorizer.
- **Turned off later:** stops the node, disconnects peers and keeps the data. It asks whether to keep or delete keys and certificates, and warns that deleting the master's wraps can't be undone.

#### 7.14.2 First run, joining other devices, and discovery

- **First-run screen:** "New here / Restore from a backup / Join my account on another device". For a new user with decentralization on, the steps are:
  1. Sign in to the site with the email code.
  2. Choose the mandatory password and write down the 24 words.
  3. Split the seed into 5 shares. Shares may be postponed but the phrase may not, and a banner nags until the shares are done.
  4. The database is created encrypted and the first site sync runs.
  5. This node becomes the first authorizer.
- **Pairing code (new):** on an existing node, Nodes › Add device shows a short **single-use code (also a QR)** that expires after 10 minutes. It carries the `acp-id`, a one-time rendezvous key and several relay hints.
  - The new device chooses "Join my account" and enters the code. It then finds the approver through the LAN, a relay or Kademlia (looked up by the rendezvous key) and sends its enrollment request over libp2p.
  - Headless machines (servers, DGX Sparks) run `acp-rs --join CODE` over SSH. They print the verification words, and the user approves on a node with a screen.
- **The approver sends the signed certificate to the new node** (§7.13.3 step 4), inside the enrollment bundle over the same libp2p stream, or as a pasteable string when there is no network path.
  - The certificate is bound to the new node's public key and to the request nonce, so it can't be reused for another node.
  - The new node verifies the chain against the `acp-id` the user confirmed.
  - If the connection drops, the node repeats the same request and the approver **re-sends the same certificate**. It never issues a second one for one request.
- **Syncing between your nodes needs no per-pair setup.** A certified node learns its siblings from the signed `NodeEnrolled` records and their last known addresses, and connects directly, through a relay, or by Kademlia lookup by `acp-id`.
- **Discovering other users' nodes:** by invite code, bootstrap peers, or the LAN (below).
- **mDNS is on by default** (this replaces "off by default" in §7.3).
  - Discovered nodes appear in a **Nearby nodes** list, and nothing connects or syncs automatically.
  - For each one the user picks **Add to my account** (the enrollment flow, for an unenrolled node) or **Sync as a foreign node** (a chosen trust level: `pull-only`, `exchange` or `trusted`, within the allowlist).
  - mDNS announces only a random per-session id and the network name, never the `acp-id`, nick or label.
  - Unknown peers are still disconnected after `identify` (§7.4).
  - The user's own already-enrolled nodes reconnect over mDNS without a prompt, because the certificate chain proves they belong to the user.
  - mDNS can be turned off in settings.

#### 7.14.3 Relays

- **A relay crate:** `acp_rs_node_relay`, usable in two ways: as a library that any node embeds (`--serve --relay`), and as a standalone small binary, `acp-rs-relay`, that can run on a cheap VPS.
  - It provides libp2p relay v2, AutoNAT and a Kademlia rendezvous, and acts as a bootstrap peer.
  - It is **blind**: it forwards the encrypted Noise or QUIC stream only. It holds no keys, certificates or data, and logs counts, never addresses or peer ids.
  - It enforces limits on reservations, circuit length and bytes per peer and per network, optional allowlists (by `acp-id` or network name), and takes a plain config file.
  - Relays share their addresses with each other through Kademlia. Pairing codes carry several relay hints.
- **Relaying is on by default, in two scopes** (both can be turned off):
  - **Local:** every node relays on its own network and for peers it can already reach directly. This works behind NAT.
  - **Public:** a node that AutoNAT finds publicly reachable also relays for the wider network, with conservative limits (a handful of reservations, short circuits, a daily byte cap). A public content node (`--serve --public`, §7.4) turns this scope on so strangers can dial it. The scope can also be on by itself, without publishing that node's database.
- A default public relay run by the project is **not** assumed. The network relies on the nodes themselves, and users may also run their own `acp-rs-relay`.

#### 7.14.4 Node roles (opt-in)

- Every node starts as a **plain, equal node**: it syncs what its storage allows and takes no special duty.
- Roles are turned on only by the user, in the Nodes list or with a flag:
  - **publisher** (holds the site session and publishes intents, §7.6)
  - **scraper**, with a **lease** so two nodes don't scrape at once
  - **limited**, with a storage cap or a history window (for laptops)
- The first node is also the authorizer, because it must certify the others. That is the only default role, and the user can move it.
- Allow **a second authorizer**, signed by the master, so certificate renewal doesn't depend on one machine (§7.13.4).

#### 7.14.5 Operations

- **Service install on every platform:** `acp-rs --service install|uninstall|start|stop|status` chooses a **systemd unit** on Linux, a **launchd** agent or daemon on macOS, and a **Windows Service** on Windows. `acp-rs-relay` has the same flag.
  - Services run as a normal user, never root or SYSTEM, with the data directory in that user's profile.
  - A service can't type a passphrase. **Decided 2026-10-19:** installing asks whether to read the unlock secret from the OS keyring or to start locked and wait for an unlock. Both are supported. The keyring is a `SecretStore`: Secret Service on Linux, Keychain on macOS, Credential Manager on Windows (§7.13.6). This lands in 3.23.0 with the service install.
- **State lives in the encrypted database.** Peers, bootstrap list, relay addresses, node roles, sync state and certificates are rows, not files, so they are encrypted at rest and included in backups.
  - Outside the database there are only the small bootstrap file (the database location and the Argon2 parameters needed to unlock) and `preferences.toml` for hand-edited settings, which holds no state.
  - The old `peers.toml` becomes a one-time import into `p2p_peers`.
- **Fleet view:** one Nodes list showing last seen, version, certificate expiry, sync lag and disk use, with rename, role change and revoke from any authorizer. `--doctor` checks the local setup.
- **Version compatibility:** nodes speak the current and the previous protocol version, announce their version, and warn when one lags.
- **Peer management:** a Peers tab with status (up, down, unknown), last seen, trust level, source (invite, mDNS, bootstrap, backup), bytes in and out and notes, with actions to change trust, block, forget and re-check.
  `--peer-list`, `--peer-check` and `--peer-prune` do the same from the command line, all backed by `p2p_peers`.
- **Peers are transient.** After a restore, every peer starts as `unknown` and is checked automatically. Those that don't answer are marked `down`, retried with backoff and pruned after a period the user sets.
- **Your own certified nodes are not peers.** After a restore they are marked "unreachable since the backup" and stay valid, since revoking is a security action. They show up in the Nodes list with a one-key revoke, and the user gets a reminder if one stays unreachable for about 60 days. That nag is not the certificate lifetime. A node certificate lasts 365 days (§7.13.2).

#### 7.14.6 Changes to restore and to key handling (amends §7.13.6 and §7.13.7)

1. **Restore lists the backup's node list** and marks them as above. It offers to revoke those the user knows are gone, which starts a new key epoch.
2. **A restored node can become the new authorizer.** The master signs that certificate in the same ceremony. Without an authorizer nobody can renew certificates.
3. **Epochs created after the backup:** their wraps to the master exist in the network, but the master is erased after the ceremony. Either a short catch-up re-wrap ceremony with the master, or any reachable authorizer re-wraps them for the new node.
4. **Two kinds of data keys instead of one shared database key:**
   - a per-node **local at-rest key**, random and wrapped by that node's unlock passphrase, which encrypts the database on that machine
   - an **account data key** per epoch, used only for account-private data (bookmarks, research, notes, feeds, drafts) and for backups, wrapped to every authorized node and to the master
   A lost node then doesn't force re-encrypting everything, only a new account epoch.
5. **Master but no backup:** identity is recovered, but private data survives only on nodes that are still alive. The restore screen says so.

#### 7.14.7 Crate layout, the FFI surface and the Dart SDK

- **Crates:** `acp_rs_node_core` shipped in 3.2.0 and is a normal dependency of the terminal app (not behind `p2p`). It holds identity crypto (the master, the phrase, SLIP-39), canonical CBOR, HPKE wraps of the account key, and a `SecretStore` interface. A later relay-only package, the tray, and `acp_rs_node_ffi` depend on it. The remaining crates stay later, and behind `p2p` when they need the network: `acp_rs_node_identity` (enrollment and revocation over the network; the local certificate types land in the core in 3.3.0), `acp_rs_node_store` (moving the encrypted database and backups out of the terminal crate), `acp_rs_node_net`, `acp_rs_node_sync`, `acp_rs_node_relay` (§7.14.3) and **`acp_rs_node_ffi`**.
- **`acp_rs_node_ffi`** is the single stable C-compatible surface for every other language.
  - It uses opaque handles and versioned structs, explicit memory ownership and error codes, and an async callback or event-stream interface for sync events.
  - It is tested for struct versioning, errors across the boundary, ownership, and calls from background threads.
- **Dart SDK** (`acp_rs_node_dart_*`): a thin package that calls the embedded Rust crates through `acp_rs_node_ffi` (FFI). There is **one implementation of the protocol**, so a backup made on one platform restores on another by construction.
  - Packages: a low-level binding package (generated from the C header) and a high-level package with async Dart APIs and streams for identity, enrollment, peers, sync, drafts and review.
  - Platform builds: Android, iOS, macOS, Windows, Linux, with prebuilt binaries per target.
  - The Dart side handles the UI, platform integration (Keychain/Keystore, biometric unlock, background tasks, notifications) and the lifecycle (foreground sync on iOS).
  - **Libp2p-only app:** no site session, no scraping or publishing. Its content comes from peers, so the network needs at least one node that scrapes. Its first run is "create account or join with a pairing code", a storage cap, then peers through pairing, invites or mDNS.
  - Cross-implementation vector tests are optional (only if a separate implementation is ever written).
- **Protocol spec:** [`docs/PROTOCOL.md`](../docs/PROTOCOL.md) plus test vectors. 3.2.0 writes the canonical CBOR rules. 3.3.0 adds the certificate records and the PeerId encoding. The envelope and the request-response protocols are later. The `.acpbackup` header is format version 1 and is not this map encoding. **No protobuf**: signing and hashing stay on canonical CBOR (§7.5).

#### 7.14.8 Local-first posts, drafts, versions and review

- **Local-first:** a post is written to the database first. The site is one place it can go afterwards.
  - Record: `LocalPost{id (ULID), acp-id, body_json, state, versions, site_post_id?}`.
  - States: `draft`, `local`, `queued` and `published`. A user may have any number of drafts.
  - Without the site, a post stays `local`: it is in the database and it replicates to the user's own nodes. A node with a site session (a publisher) turns it into an intent with the usual preview and confirm, publishes it, reads it back, and links the local id to the site post id.
  - Per post, the user chooses **keep local** or **publish to the site**. For a libp2p-only app, keep local is the default.
- **Versions:** every save of a draft creates a new version, and so does every edit after publishing.
  - Table `post_versions` (§2.3) holds them, with `source` ∈ `site`, `peer`, `local`, `draft`.
  - The versions the site already keeps (feature 14, `/rs/post/{thread}/{id}/edits`, P1 version 2.24.0) are captured into the same table with source `site`.
- **Privacy:** drafts are private. They are never visible publicly until published.
  - Each draft has its own random key. Its versions are stored and replicated as **encrypted records**, first to the user's own nodes (wrapped under the account data key).
  - Ciphertext goes to other peers **only for a draft with an active review link**, since the existence, size and timing of a draft are still metadata. Peers who can't read it can carry it.
- **Review:**
  - A link `acpdraft1:` holds the draft id, the key and the sender's relay hints, so it only works through decentralized nodes. It comes in two kinds: view-only, and comment-and-suggest.
  - Reviewers sign comments with their own `acp-id` (or a throwaway key). Comments attach to a version and a text range.
  - Suggested changes arrive as a proposed version branching from the author's, shown as a diff. The author accepts or rejects each one, and accepting creates a new version of the author's own draft. Reviewers can't change the author's versions directly.
  - Revoking a link rotates the draft key for later versions. It can't undo what someone already saw, and the UI says so.
  - **Reviewers' comments stay private, but are retained** with the draft, and are never published with the post.
- **Publishing:** it follows the normal preview and confirm.
  - Once published, **all versions become public** through the decentralized nodes (decrypted records under the same post id). A notice at publish time shows how many versions will be released.
  - The site only has the final text, so earlier versions exist on the network only.
  - The UI gets a version picker ("version 3 of 7, edited by ...") and a clear marker for versions that came from a draft instead of the site.

#### 7.14.9 Roadmap changes

- **3.0.0** is identity, encryption at rest, and backup/restore (§7.13, without a node). It does not depend on P2P and it has shipped. **3.1.0** is the job-details window and has shipped. **3.2.0** is `acp_rs_node_core` and has shipped. **3.3.0** is local node identity and has shipped. The write core is 3.4.0. Writes stay off until then. Every later feature is one minor (§4).
- **Local-first posts, drafts and `post_versions`** land with the Outbox in P6 (3.16.0).
- **3.22.0** is password change / re-key (§7.13.13). It does not change the master or the `acp-id`. It is before the daemon.
- **3.23.0** adds, beyond what 3.0.0 already shipped: the relay crate and `acp-rs-relay`, mDNS on by default with Nearby nodes, the pairing code and `--join`, peer management, the cross-platform service install, the node roles, authorizer and node certificates, and OS keyring unlock for the daemon.
- **3.28.0:** draft review links, comments and suggestions.
- **3.29.0:** `acp_rs_node_ffi` and the first Dart SDK release, followed by Dart-only releases.
- **Trays and version display** (§7.14.10): the version display is complete from the first release that has the node. `acp-rs-tray` ships with 3.23.0 together with the service install. The TUI header already shows `v<version>` from 3.0.0.

#### 7.14.10 Version display and desktop trays

- **Version in the UI:** the TUI header already shows `v<version>` (from `CARGO_PKG_VERSION`). Make that complete everywhere:
  - the header, an About screen (version, build date and commit, protocol version, whether the `p2p` feature is built in), `--version` and `--node-status`
  - the node chip in the header, the Nodes list for every node (§7.14.5, which also warns about lagging versions), the relay binary's `--version`, and the tray menu header
  - every window title of the tray apps
  - **Versioning (revised 2026-10-19):** **3.0.0 is the major release.** It is identity, encryption at rest, and backup/restore, and it breaks a 2.x plaintext database. The whole plan stays in the 3.x line. Following the rule in §4, 3.1.0 is the job-details window, 3.2.0 is the shared core, 3.3.0 is local node identity, 3.4.0 is the first write feature, and every later feature is one minor: interactions 3.5.0–3.9.0, live 3.10.0–3.11.0, account 3.12.0–3.15.0, outbox 3.16.0, feeds 3.17.0–3.20.0, spaces 3.21.0, password change / re-key 3.22.0, the network node 3.23.0–3.27.0, review 3.28.0, the Dart SDK 3.29.0 (§7.11). There is no 4.0 in this plan. Inserting 3.22.0 moved each later stage up one minor.
- **Tray apps** to control the node and the relay without a terminal: a small separate binary, `acp-rs-tray`, in its own crate, that talks to the node and relay over the local control socket (§7.2).
  - **Menu:** status (running, stopped, locked, peers online, sync state, version), Start, Stop and Restart for the node and for the relay, **Unlock** (prompts for the passphrase when the node started locked), Open TUI, Nearby nodes and pending enrollments (approve from the tray), Pause syncing, Copy `acp-id`, Open logs, Quit.
  - **Notifications** for a pairing request, a new node enrolled, a certificate close to expiry, a revocation received, and a sync error. Each is optional.
  - **Platforms:**
    - **Linux, KDE Plasma:** StatusNotifierItem tray (works natively).
    - **Linux, Hyprland:** StatusNotifierItem, shown by the user's bar (for example Waybar's `tray` module). Setup notes in the docs.
    - **Linux, GNOME:** needs the AppIndicator/StatusNotifier shell extension, because GNOME has no tray by default. The tray detects its absence and falls back to a notification plus `acp-rs --service status`, and the docs explain the extension.
    - **Windows 11 and later:** notification-area icon, with the overflow behaviour of Windows 11 documented.
    - **macOS:** menu bar item (`NSStatusItem`).
  - The tray starts with the login session through the platform mechanisms in §7.14.5 (XDG autostart, a Windows Run key or Startup shortcut, a macOS login item). It stays optional.
  - The tray never holds keys. It sends commands, and unlocking goes through the node's own prompt.

#### 7.14.11 Relay-only packages

The repository already packages the app for Debian (`debian/`), Arch (`packaging/arch/PKGBUILD`) and Fedora (`packaging/fedora/*.spec`). The relay (`acp-rs-relay`, §7.14.3) gets **its own packages**, so a VPS or a spare machine installs the relay alone, with no TUI, tray or scraper.

- **One package per platform, named `acp-rs-relay`**, containing only the relay binary, its default config, the service definition, the licence and the docs:
  - **Debian and Ubuntu:** a `.deb`, built from a second source package in `debian/` (or a separate `debian-relay/`), with a systemd unit, a dedicated `acp-relay` system user, `/etc/acp-rs-relay/config.toml` as a conffile and a post-install step that enables but doesn't start the service.
  - **Arch:** a `PKGBUILD` in `packaging/arch/relay/` (systemd unit, `sysusers.d` and `tmpfiles.d` entries, `backup=()` for the config).
  - **Fedora:** a `.spec` in `packaging/fedora/relay/` (systemd macros, `sysusers` entry, `%config(noreplace)`), built as an RPM.
  - **macOS:** a signed and notarized `.pkg` that installs the binary and a launchd daemon plist, supporting Apple Silicon and Intel, with a universal binary. A Homebrew formula is part of the same publishing decision below, not an earlier channel.
  - **Windows 11 and later:** an `.msi` (WiX) that installs the binary, registers the **Windows Service**, adds the firewall rule for the relay port and writes the default config under `%ProgramData%`. A `winget` manifest points to the same installer.
- **Same defaults on every platform:** the relay starts with conservative limits (§7.14.3), a config file, log counts only, and its own data directory (a small state file for its identity key, kept as in §7.13.6). It runs as a service user, never root or SYSTEM.
- **Not in the package:** the app, the tray, any keys or certificates of a user. The relay has its own node key only and no account.
- **Build and release:**
  - CI builds each package from the tagged release for `x86_64` and `aarch64` where the platform supports it, with reproducible build settings.
  - Packages are built as release assets with checksums and a detached signature when a release is cut. That is not the same as publishing them to a distribution repository.
  - Package smoke tests in CI: install in a clean container or VM, start the service, check that it listens, uninstall, and confirm that nothing is left behind.
  - The package version is the release version (§7.14.10), shown by `acp-rs-relay --version`.
- **Publishing (decided 2026-10-19, §8 Q14).** APT, Fedora COPR, the AUR, and the macOS Homebrew formula are published only once the full set exists: decentralized publishing (3.25.0), replies (3.4.0), review (3.28.0), the node crates (through 3.27.0), and the Dart package (3.29.0). Until that set has shipped, packaging in this repository stays unpublished. Someone can still build a package by hand from the tree.
- **Roadmap:** the relay crate lands in 3.23.0 (§7.14.9). It is not published to APT, COPR, AUR, or Homebrew in that release. Those repositories wait for the full set above.

---

## 8. Open questions for Michael

1. ~~**ToS / Party policy:** is automated posting acceptable? And is redistributing scraped Red Star content between
   members' nodes acceptable, given the site is members-only?~~
   **Resolved 2026-10-18.** Red Star has no forum rules beyond "don't be a bad communist". It's party-run and disciplined,
   so posting from the client and redistributing content between nodes are both allowed.
2. **Priorities:** which writes first? The proposal is reply → new post → vote → bookmark (done) → subscribe → edit/delete → repost.
3. ~~**Capture:** may I capture the real websocket frames once?~~
   **Resolved 2026-10-18.** It'll be a guided session: Michael writes one real reply himself, and its LiveView frames are captured with secrets
   redacted. There's no dev-tools export. The procedure is in §4.1.
4. ~~**Notifications:** may I open `/rs/notifications` even if that marks them read?~~
   **Resolved 2026-10-18.** Yes. It's opened in P0 (§4.1) and used in P4.
5. **View counting:** should the client send `view-post` like the browser does (honest view counts), or never (privacy)?
6. **Spaces:** skip them, just list them, or try audio (needs LiveKit and an audio stack, a big lift)?
7. ~~**Writes on by default from the write core (3.4.0)?**~~ **Resolved 2026-10-18.** Yes, they're on by default once writes exist. 3.0.0 does not add writes. The per-write preview and confirm (Cancel default) and `--dry-run` stay,
   and the opt-out is `ui.writes = false` or `ACP_WRITES=off` (§3).
8. **Outbox:** auto-send queued items when you're back online, or always confirm again?
9. **Feeds:** is one level of rule grouping enough? Should shared definitions also be allowed to include pinned post ids?
10. **P2P:**
    - ~~Should nodes relay writes to the site?~~ **Resolved 2026-10-18.** Yes. Nodes sync to the site too. Writes are published only with
      the author's own session, through intents (§7.6). "Private-only, never relay writes" was rejected.
    - Remaining sub-questions on writing to the site:
      - Allow several publisher nodes per account, or exactly one?
      - When the publisher is offline, may third-party peers store and forward *sealed* intents, or should intents only travel
        directly or through relay circuits?
      - Should destructive actions (delete, username change) need a second confirmation on the publisher?
      - How long can an intent wait before it expires (proposed: 24 h, matching the outbox)?
      - Should a node↔account binding be verifiable by others, for example a node id in the Red Star bio? That would publicly link a node to a nick.
    - Scope: posts only, posts + media, or also feeds/research when opted in?
    - ~~Networks: private and invite-only by default, and should public nodes exist?~~ **Decided 2026-10-19.** The default stays private and invite-only. Public nodes exist and are opt-in (`--serve --public`). What that means for relay scopes, the node record, and the stage order is in §7.4. The flag lands in 3.23.0. Kademlia advertisement lands in 3.24.0.
    - ~~Should peer tombstones delete local copies, or only flag them?~~ **Revised 2026-10-19.** This supersedes the earlier note that dropped the body after a grace period. A signed tombstone keeps the post data. Never let data go to waste. The post is shown as deleted with a tombstone icon (🪦, ASCII fallback `[RIP]`), and the content stays viewable. If the author publishes again, re-signs the same post id, or retracts the tombstone with a signed restore record, the post id is restored and the tombstone is cleared. The restore record is signed by the author's key, references the tombstone, and a newer timestamp wins (§7.5, `docs/PROTOCOL.md`). The icon is part of the interactions and feeds UI (§1b, §6.6).
    - Is an MSRV bump OK when libp2p 0.58 needs it?
    - ~~Use 4.0.0 for decentralization, or keep it in 3.x?~~ **Resolved 2026-10-18.** It stays in 3.x, as 3.23.0 and later (§7.14.10).
11. **Keys, identity and backup (§7.13). Resolved 2026-10-18:**
    - ~~Backup and restore~~ is designed (§7.13.7).
    - ~~Email as a salt~~: no. The seed is random and the email is never an input (§7.13.1).
    - ~~Password~~ is mandatory, with no passwordless mode.
    - ~~Recovery~~ is SLIP-39 Shamir, 3 of 5, with the password kept outside the shares (§7.13.8).
    - ~~Node keys~~: independent keys with certificates, plus an authorizer certificate so the master can stay in a safe (§7.13.2 to §7.13.5).
    - ~~Encryption~~: the database is encrypted at rest, with envelope encryption wrapped to every node and to the master. Replication sends plaintext inside the encrypted libp2p link and HTTPS for the site (§7.13.6).
    - ~~Identity~~ is pseudonymous. A link to a nick is optional (§7.13.9).

    Resolved with 3.0.0:
    - ~~**Encryption scope:**~~ SQLCipher for the whole database. Column encryption was rejected because cached posts would stay readable.
    - ~~**Timing:**~~ encryption at rest and backup/restore shipped as 3.0.0. They do not wait for the node.
    - ~~**Local passphrase:**~~ at init it is the same string as the account password. Rewrapping that local key does not rotate the master or the `acp-id`.

    Decided 2026-10-19:
    - ~~**Default lifetimes:**~~ confirmed. An authorizer certificate lasts 30 days, a node certificate lasts 365 days, and clock skew is 5 minutes (§7.13.2, §7.13.4). Renewal, the 7-day grace period, and revocation are still to be built in their planned stages (3.23.0).
    - ~~**History access on the first machine:**~~ accepted. `history_grant` is `allowed` and `max_nodes` is 16. A later enrollment still asks the approver (§7.13.3).
    - ~~**Daemon unlock:**~~ yes. `--serve` and the service install may read the unlock secret from the OS keyring, behind `SecretStore`: Secret Service on Linux, Keychain on macOS, Credential Manager on Windows. Starting locked and waiting for an unlock stays available. This lands in 3.23.0, after password change / re-key (§7.13.6, §7.14.5).
    - ~~**Account password:**~~ a password change does **not** change the master or the `acp-id`. It re-keys local data only (§7.13.13), as its own minor, **3.22.0**, before the daemon and the keyring.

    Still open:
    - **Backups:** are the exclusions right (no `session.json`, no node secret key)? May peers hold encrypted backup blobs for each other?
    - **Review:** the key derivation and the certificate formats should get an external review before 3.23.0 ships. 3.0.0 did not have that review.
12. **Onboarding, operations, Dart SDK, drafts and review (§7.14). Resolved 2026-10-18:**
    - ~~Decentralization~~ is optional, and a checkbox that is on by default (§7.14.1).
    - ~~mDNS~~ is on by default, with a Nearby nodes list, no automatic connection and no identifying announcements (§7.14.2).
    - ~~Relays~~: a separate `acp_rs_node_relay` crate, usable embedded or standalone, relaying on by default in a local and a public scope (§7.14.3).
    - ~~Node roles~~ are opt-in. Every node starts equal (§7.14.4).
    - ~~Service install~~ covers Linux (systemd), macOS (launchd) and Windows (Windows Service) (§7.14.5).
    - ~~State~~ lives in the encrypted database, with only a bootstrap file and `preferences.toml` outside it (§7.14.5).
    - ~~Peers~~ are transient and checked automatically after a restore, with a Peers tab and `--peer-*` commands. Own nodes are never revoked automatically (§7.14.5).
    - ~~The certificate~~ is returned to the new node by the approver inside the enrollment bundle (§7.14.2).
    - ~~Dart SDK~~ is a thin package over FFI to the embedded Rust crates, with a separate `acp_rs_node_ffi` crate (§7.14.7).
    - ~~Protocol spec~~ is a written `PROTOCOL.md` with test vectors on canonical CBOR. No protobuf.
    - ~~Writing without the site~~: posts are written to the local database first, with states `draft`, `local`, `queued` and `published` (§7.14.8).
    - ~~Drafts~~ are private and versioned, replicate as encrypted records to the author's own nodes, and can be shared for review by a link that works only through the nodes.
    - ~~On publish~~ all versions become public, reviewers' comments stay private but are retained, and the site's existing edit history is captured too.

    Still open:
    - Should ciphertext of a draft with a review link replicate to third-party peers (proposed, since it leaks only existence, size and timing), or only travel between the author and reviewers directly?
    - Review links: view-only and comment-and-suggest only, or also a per-link expiry by default?
    - How are the Dart SDK's prebuilt binaries built and distributed (a CI matrix, and pub.dev with native assets)?
    - Does a libp2p-only user with no publisher ever get a way to post to the site (for example by pairing with a friend's publisher)? Proposed answer: no.
13. **Versions (§7.14.10). Revised 2026-10-19.** 3.0.0 is the major release: identity, encryption at rest, and backup/restore. 3.1.0 is the job-details window. 3.2.0 is the shared core. 3.3.0 is local node identity. The plan stays in 3.x. The write core is 3.4.0. Password change / re-key is 3.22.0. The network node and trays are 3.23.0 and later. Review is 3.28.0. The Dart SDK is 3.29.0. There is no 4.0.
    The version is shown everywhere in the UI, and the trays cover KDE, Hyprland, GNOME, Windows 11 and macOS.
14. **Relay packages (§7.14.11).** Decided: `acp-rs-relay` ships as its own package for Debian, Arch, Fedora, macOS and Windows 11 and later.
    **Publishing decided 2026-10-19.** APT, COPR, the AUR, and the macOS Homebrew formula are published only once the full set exists: decentralized publishing, replies, review, the crates, and the Dart package (3.4.0 through 3.29.0, §7.14.11). Until then, packaging stays unpublished.
