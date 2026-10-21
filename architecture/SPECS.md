# TUI

Let's add a tui mode for searching and reading the posts in the db. This should be the default view if no flags are passed in.


## Features

- I should be able to see clickable nicks on the left bar with their count of posts
    - When I click on them, I should see posts in a scrollable view in the body
- The topbar should have the name ACP Red Star
- I should have a list of clickable commands in the menu bar right below the top bar
    - Options that work with that command should be displayed with the label of the option and support these types on the right of the label:
        - Checkbox for boolean flags
        - Dropdown selector for know choice values
        - Textfield for open ended values
        - Button for actions that need to be taken
    - All commands come from flags the cli supports
        - Options are considered flags that can or must be used with a selected flag
    - A Start Scanning button will always be to the right of the commands
- I should have a help modal with all the usage information for the commands
- I should be able to click on a post in the body and see the post details
    - The post details show under the body and the body slides up to take a small, but visible amount of space
    - The post should be as detailed as the html format is
    - Links in the posts should cross load other posts
        - This should animate in the same way as the post loading; only pushing the previous post detail up a bit
- I should be able to manage sessions
    - Log in
        - The screen should just show a login modal until authentication is complete
    - Log out
        - the screen routes to log in when log out is successful

Use the following layouts below as needed.

### Post animation and layout details

Let's talk about how this looks like for a second. Here are some examples:

#### The Base Layout

```
+--------------------------------------+
| <Topbar>                             |
+--------------------------------------+
| <Menu Bar>                           |
+--------------------------------------+
| <Nicks>    | <Body>                  |
+--------------------------------------+
```

#### Menu Bar

```
+--------------------------------+-------------------------+
| <item 1> | <item 2> | <item 3> | <Start Scanning button> |
+--------------------------------+-------------------------+
```

#### Menu Bar with item clicked

```
+----------+============+----------+-------------------------+
| <item 1> | <item 2>   | <item 3> | <Start Scanning button> |
+----------|============|----------+-------------------------+
           | <Option 1> |
           | <Option 2> |
           | <Option 3> |
           +============+
```

#### Menu Bar with item clicked; option details displayed

```
+-----------+=======================+----------+-------------------------+
| <item 1>  | <item 2>              | <item 3> | <Start Scanning button> |
+-----------|=======================|----------+-------------------------+
            |  [ ] <label 1> <type> |
required -> | *[x] <label 2> <type> | <- selected
            |  ...                  |
            +=======================+
```

#### Menu Bar with item clicked; option details displayed; option types displayed

```
+----------+==============================+----------+-------------------------+
| <item 1> | <item 2>                     | <item 3> | <Start Scanning button> | <- Starts the scan
+----------|==============================|----------+-------------------------+
           |   [ ] <label 1> <empty>      | <- A boolean type is empty because the select checkbox will determine true or false
           | * [x] <label 2> <drop down>  | <- For when we have known values to select from
           |   [ ] <label 3> <text field> | <- For dynamic values
           |   < <label 4> button       > | <- Will be clickable to take direct action
           +==============================+
```

#### Modals

Modals can be used for various things, such as:

- Collecting information needed to complete an action
- Display things like help over the layout
- Etc.

They are considered floating above the rest of the layout and can take up all of the available space, sans a small margin.

All modals have actions to deal with closing it with a certain state. These states are:

- CLOSED_CANCELLED
- CLOSED_SUBMITTED

#### Posts loaded into the body

```
+--------------------------------------+
| ...                                  |
+--------------------------------------+
| ...                                  |
+--------------------------------------+
| +========+ | +---------------------+ |
| | Nick 1 |>| | <Post 1>            | |
| +========+ | +---------------------+ |
| | Nick 2 | |                         |
| +--------+ | +---------------------+ |
| | Nick 3 | | | <Post 2>            | |
| +--------+ | +---------------------+ |
|            |                         |
|            | +---------------------+ |
|            | | <Post 3>            | |
|            | +---------------------+ |
|            |                         |
|            | +---------------------+ |
|            | | <Post 4>            | |
|            | +---------------------+ |
|            |                         |
|            | +---------------------+ |
|            | | <Post 5>            | |
|            | +---------------------+ |
+--------------------------------------+
```

#### Post details loaded into the body

```
+--------------------------------------+
| ...                                  |
+--------------------------------------+
| ...                                  |
+--------------------------------------+
| +========+ | +---------------------+ |
| | Nick 1 |>| | <Post 1>            | |
| +========+ | +=====================+ |
| | Nick 2 | | | <Post 2 details>    | |
| +--------+ | | ...                 | |
| | Nick 3 | | | ...                 | |
| +--------+ | | ...                 | |
|            | | ...                 | |
|            | | ...                 | |
|            | | ...                 | |
|            | | ...                 | |
|            | +=====================+ |
|            | | <Post 3>            | |
|            | +---------------------+ |
|            | | <Post 4>            | |
|            | +---------------------+ |
|            | | <Post 5>            | |
|            | +---------------------+ |
+--------------------------------------+
```

#### Post details from linked posts

First link clicked:

```
+--------------------------------------------+
| ...                                        |
+--------------------------------------------+
| ...                                        |
+--------------------------------------------+
| +========+ | +---------------------------+ |
| | Nick 1 |>| | <Post 1>                  | |
| +========+ | +===========================+ |
| | Nick 2 | | | <Post 2 details>          | |
| +--------+ | | +=====================+ | |
| | Nick 3 | | | | <linked post details> | | |
| +--------+ | | | ...                   | | |
|            | | | ...                   | | |
|            | | | ...                   | | |
|            | | | ...                   | | |
|            | | +=====================+ | |
|            | +===========================+ |
|            | | <Post 3>                  | |
|            | +---------------------------+ |
|            | | <Post 4>                  | |
|            | +---------------------------+ |
|            | | <Post 5>                  | |
|            | +---------------------------+ |
+--------------------------------------------+
```

Link inside linked post clicked:

```
+--------------------------------------------------+
| ...                                              |
+--------------------------------------------------+
| ...                                              |
+--------------------------------------------------+
| +========+ | +---------------------------------+ |
| | Nick 1 |>| | <Post 1>                        | |
| +========+ | +=================================+ |
| | Nick 2 | | | <Post 2 details>                | |
| +--------+ | | +=============================+ | |
| | Nick 3 | | | | <linked post details>       | | |
| +--------+ | | +=============================+ | |
|            | | | <other linked post details> | | |
|            | | | ...                         | | |
|            | | | ...                         | | |
|            | | | ...                         | | |
|            | | +=============================+ | |
|            | +=================================+ |
|            | | <Post 3>                        | |
|            | +---------------------------------+ |
|            | | <Post 4>                        | |
|            | +---------------------------------+ |
|            | | <Post 5>                        | |
|            | +---------------------------------+ |
+--------------------------------------------------+
```

So as you can see, there's a sliding -> nesting -> sliding layout happening here. The body should be able to handle this gracefully.


---

# Design expansions & suggested changes

These are proposed additions and clarifications on top of the layouts above.
They are **not implemented yet**. They exist so 2.0 can ship with a coherent
default, a stable CLI↔TUI contract, and testable behavior.

## Versioning

- **Ship as 2.0.0** (major). Default with no flags becomes the TUI instead of
  "prompt for nick(s) and scrape". That is a breaking change for scripts that
  relied on the interactive scrape prompt.
- Keep every existing flag working in non-TUI (headless) mode. Scripts that
  already pass `--nick`, `--search`, `--export`, etc. keep their behavior.
- Bump minor for later TUI-only polish after 2.0 lands.

The client has moved on from this 2.0 note. Text posts and replies are sent (plan 3.35.0): validate, then save, once, from the command line and the compose box. Plan 3.36.0 uploads the site copy first (images, videos, and PDFs, at most 4), using the captured `allow_upload` / `lvu` channel. `--dry-run` prints the frames and the upload plan and does not connect. Votes, subscribes, deletes, edits, and reposts are not sent. The crate version stays 3.5.0. The hand check is [docs/LIVE-PUBLISH-TEST.md](../docs/LIVE-PUBLISH-TEST.md). The upload notes are [UPLOAD-CAPTURE.md](./UPLOAD-CAPTURE.md).

## Default entry & CLI contract

| Invocation | Behavior |
|---|---|
| `acp-red-star` (no flags) | Open the TUI (read DB offline first; login only when a scan needs it) |
| `acp-red-star --tui` | Force TUI even if other offline flags are present (optional alias) |
| Any scrape / search / export / import / maintenance / generate flag | Current headless CLI path (unchanged) |
| `acp-red-star --help` / `-V` / `--generate` | Unchanged |

**Suggested change:** add an explicit `--scrape` (or keep "any scraping flag
implies scrape") so headless scrape stays reachable without relying on the
old "empty argv → prompt" path. The TUI's **Start Scanning** button builds the
same `Args` the CLI would have used and runs that path in-process.

## Session management (auth)

Today the cookie jar is **in-memory only**; only the email lives in
`~/.acp/preferences.toml`. For a TUI that can open, browse offline, then scan:

**Suggested changes:**

1. **Persist the session** under `~/.acp/` (e.g. `session.json` or a cookie jar
   file). Load on TUI start; treat "GET /rs does not redirect to /auth" as logged in.
2. **Login modal** (per the layouts): blocks the main chrome until
   `CLOSED_SUBMITTED` (verified) or the process exits. Fields: email (pre-filled
   from prefs), verification code / paste-link. Reuse the existing LiveView login.
3. **Logout**: clear the persisted jar + in-memory jar, then route to the login
   modal (`CLOSED_SUBMITTED` on logout → login). Prefer hitting the site's
   logout URL if we can discover it; otherwise local clear is enough for "logged
   out of this tool".
4. **Offline-first default:** opening the TUI does **not** require login. Login
   is required only before **Start Scanning** (or an explicit Login menu item).
   Browsing nicks/posts from the local DB works while logged out.
5. Menu items: **Login** / **Logout** (Logout disabled or hidden when logged out).

## Menu bar ↔ CLI flag map

"All commands come from flags the CLI supports" — map clap headings to menu
commands. Options under a command are the flags that combine with it.

| Menu command | Primary flag / action | Options (widgets) |
|---|---|---|
| **Scrape** | Start Scanning runs scrape | Mode: Latest / Trending (dropdown); Nicks (text); Mentions (checkbox, default on); Said-by (checkbox); Crawl limit (text); Attachments (checkbox, default on); Full (checkbox) |
| **Search** | offline search into Body | Query (text); optional Nick filter (text) |
| **Export** | dump DB / current Body selection | Path (text); Format json/html (dropdown); Name (text); Compress + tar.gz (checkbox + implied format) |
| **Import** | import file into DB | Path (text); Compress tar.gz (checkbox) |
| **List** | refresh left Nick bar from DB | (none — or Format for a one-shot dump) |
| **Maintain** | destructive DB ops | Remove nick (text); Keep replies (checkbox); Clear all (button → confirm modal); Dry-run (checkbox); Yes skip-confirm (checkbox, dangerous) |
| **Session** | Login / Logout | Login (button → modal); Logout (button) |
| **Help** | usage modal | (none) |
| **Quit** | leave TUI | (none) |

**Start Scanning** stays pinned right (per layouts). It is enabled when Scrape
options are valid (at least one nick, or a confirmation to scrape from the
currently selected nick in the left bar).

The bar is View, Search, Compose, Library, Bookmarks, Research, Sync, Discover,
Account, and Debug, then Help, About, and Quit, with Scrape immediately left of
Start Scanning. Bookmarks is its own menu (Show, Bookmark current, Sync with
acp.us). Library opens Export, Import, Basket, List, and Maintain. Search
history joins Search (a past query, re-run when picked) in its own minor when
Search work is touched. Sync keeps the local copy up to date. Discover finds
people, nodes, and rooms. Those stay separate menus. The table above is the
older flag map. The layout is in [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) §4.2.

Every actionable control shows a short tooltip on hover, including menu-bar entries and the icons on a card or detail. When the control has a key, the tooltip includes it (`Bookmark (b)`). The next line is `? {topic}` and opens the Help topic for that action. One shared helper draws them. Text posts and replies are sent. Uploads and interactions stay behind `CAPTURE_BLOCK`. See [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) §4.3.

Help is topics you can move between, with search inside Help. A `?` on the header opens Help. `?` and F1 stay the entry. Every user-facing flag, menu command, menu option, and control has a description in Help and a tooltip. See [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) §4.4.

About sits beside Help. The menu has three segments: Credits, Why this is necessary, and ACP platform. Choosing any segment opens the same modal, focused on that section. ACP platform is the vendored American Communist Party Program (adopted October 12, 2024, <https://acp.us/program>), with headings and scroll, and it is not paraphrased. The same how-to-join link, <https://acp.us/join>, sits at the top and the bottom of that text only. Those links are chrome, not a fourth section. Each segment has a tooltip and a Help topic, and the tooltip deep-links to that segment. Getting Started (`docs/GETTING-STARTED.md`) covers the command line, terminal prompts outside the TUI, and the TUI, and it includes About. The user-facing name in the launch docs is ACP Red Star. End-of-3.0 communications, including a cutscene in video and ASCII from public-domain or otherwise licensed archival footage, wait until the remaining launch docs exist. The comrades' forum post features that cutscene in both forms beside the pitch and the demo. A separate community-work video, chapter fieldwork rather than the cutscene or the demo, is also promo and is not produced here. Publish stays behind `CAPTURE_BLOCK`. See [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) §4.2 and §4.5.

**Suggested change:** selecting a nick in the left bar pre-fills Scrape's
Nicks field and filters the Body. Multi-select nicks (Shift/Space) are a must have
for 2.0.

**Suggested addition:** have a sync loop:

There should be an option to have the TUI sync periodically with Red Star (rs) if they are online. This will keep the database up to date.

Required options (marked `*` in the layouts) for Scrape: at least one nick.
For Export: path. For Import: path. For Search: query (empty query = all posts
for selected nick / all nicks — match current `--export` semantics).

## Body, posts, and detail stack

Parity with HTML export (`export/html.rs`):

- Meta: `@nick · posted_at` **Suggested change:** `@nick · posted_at · link_to_post_id`
- Reply line: "Replying to {in_reply_to_id}" (+ thread root when different)
- Body text (preserve newlines)
- Attachments (path/url; images shown as path + open-hint in 2.0, inline if easy)
- Cross-links: `in_reply_to_id`, `parent_post_id`, and ids mentioned in content
  that exist in the DB → open nested detail

**Detail stack (the sliding → nesting → sliding behavior):**

1. Body is a vertical list of post summaries for the selected nick (newest first
   by default; suggest a Sort dropdown later).
2. Activating a post pushes a **detail frame** that takes most of the Body
   height; the summary list shrinks to a residual strip (still scrollable).
3. Activating a link inside a detail pushes another frame on the stack; each
   previous frame keeps a thin header strip (id + nick) so the nest is visible.
4. Esc / Back pops one frame. Closing the root detail restores the full list.
5. Missing id (not in DB): show a small inline notice + optional "Scrape this
   thread" action if logged in (2.1); 2.0 can just say "not in database".

**Suggested change:** store a max stack depth (e.g. 8) and refuse deeper nests
with a status message rather than breaking the layout.

**Suggested addition:** have a navigation bar above the posts, in the body:

- To switch view mode
- To flip through the stack - <previous> / <next>

We could have other views that just the stack view.

- Our current view is a stacked/nested
    - Here are some other options
        - Infinite stack (no nesting) with or without a content pane below when a post is clicked
        - Horizontal carousel
            - Infinite
            - Fixed
        - Grid of posts
            - Content appears in a modal
            - Has close option

**Suggested addition:** add bookmarks:

Red Star already supports bookmarks. Let's start from there. Phases:

1. Scraping bookmarks from site
2. Showing the bookmarks section in the TUI
    1. Click on it to see only posts and comments that have been bookmarked
3. Allow posts to be bookmarked
    1. Shows a clickable bookmark state in each post
    2. A click bookmarks it locally (so that it works offline)
    3. A sync can then bookmark the post remotely on the site

## Left nick bar

Before 3.x is finished this bar collapses, the same pull-out as the action-history bar on the right, so the main view can use the horizontal space. The right bar lists the actions that led to the current view. Clicking an earlier action jumps back to that state. See [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) §7.14.5. The bar does not collapse today.

- Data: `list_nicks_db` → nick + `post_count` (show `@nick` and count).
- Optional: display_name as secondary text when width allows.
- Empty DB: friendly empty state ("No nicks yet — use Scrape or Import").
- After a successful scan/import/remove: refresh the bar and keep selection if
  still present.

## Input model (click + keyboard)

Layouts say "clickable". Implement **mouse and keyboard** for every action:

| Action | Mouse | Keyboard | Description |
|---|---|---|--|
| Focus panes | click | `Tab` / `Shift-Tab` (Nicks ↔ Body ↔ Menu) | |
| Select nick / post | click | `↑` `↓` `Enter` | |
| Open menu command | click | letter hint or `m` then index | |
| Toggle checkbox | click | `Space` | |
| Dropdown / text | click | `Enter` to edit, type, `Enter` commit | |
| Back / close modal | click Cancel | `Esc` | |
| Help | click Help | `?` or `F1` | |
| Quit | Quit item | `q` (with confirm if a scan is running) | |
| Search | | `Shift-s` | Opens a search modal to search by nick, keyword, post_id, and filter by nick, posted_at range, and primary/parent post. The filter can use a combination of all those options |
| Clear Search | | `Ctrl-Shift-s` | Clears the search parameters and filter |
| Search through commands | | `Ctrl+Space` | This could be used to quickly find and run a command |

**Suggested change:** document keybindings in the Help modal next to flag usage.

## Scanning inside the TUI

- **Start Scanning** runs the existing scrape pipeline on a background task
  (tokio), with progress lines in a status/footer region (reuse log levels:
  quiet/normal/verbose/debug via Debug menu checkboxes).
- Body stays readable during a scan; nick counts refresh when the scan finishes.
- Cancel: best-effort abort of the in-flight scrape (cooperative flag); 2.0 may
  only disable Start Scanning until done if abort is hard.
- Should have sync option as well; scans for new data and pushes new changes back to the site.

## Modals

Shared modal trait with results:

- `CLOSED_CANCELLED` — discard draft state
- `CLOSED_SUBMITTED` — apply (login verified, help dismissed after OK, confirm
  delete, etc.)

Concrete modals for 2.0:

1. Login
2. Help (USAGE / flag list — generate from clap `Command` help text so it
   cannot drift)
3. Confirm destructive Maintain actions
4. Error / status (scan failure, bad path)

Margin: small inset from terminal edges (1–2 cells), dim/block the layout behind.

## Tech choices (suggested)

| Piece | Choice | Why |
|---|---|---|
| TUI framework | `ratatui` + `crossterm` | De-facto Rust TUI stack; mouse support; wide examples |
| Async | existing `tokio` runtime; UI tick loop via `tokio::select!` on crossterm events + scrape join handles | Matches current binary |
| Args bridge | Build / mutate `cli::Args` from menu state; call into `app` / scrape / export functions | Single source of truth with clap |
| Module layout | `src/tui/` with `mod.rs` (app loop), `layout.rs`, `widgets/`, `state.rs`, `modals/`, `commands.rs` (flag map) | Matches existing domain modules |
| Tests | Unit tests for state transitions (stack push/pop, Args from menu); optional cucumber "TUI offline browse" with a fake backend later | Keep Gherkin site mocks for scrape; TUI logic stays pure where possible |

**Suggested change:** feature-gate nothing for 2.0 — TUI is part of the default
binary. If binary size becomes an issue later, revisit. *Agreed.*

## Out of scope for 2.0 (park for 2.x)

- Inline image rendering in the terminal (sixel/kitty) — show paths only
- Editing posts or posting back to acp.us
- Remote DB or multi-profile `~/.acp` homes

## Added into scope post for 2.0

- Multi-select nicks / bulk export from UI selection (CLI already covers bulk)
- Full scrape-cancel semantics if cooperative cancel proves messy
    - See Open Question 5.1
- Theme / color customization beyond a sensible default (ACP red accent for
  reply links, matching HTML export `#dc2626`)

## Open questions for review

1. Should **Search** replace the Body list in place, or open results in a
   temporary filter chip that can be cleared?
    1. Inplace replacement with a back button (<) to return back from the search
2. When logged out and the user hits **Start Scanning**, auto-open Login then
   continue the scan on `CLOSED_SUBMITTED` — or require Login first?
    1. Autologin will still require interaction (OTP)
    2. Open a login modal
    3. Continue scan afterwards
3. Persist session cookies: acceptable under `~/.acp/` with `0600` perms?
    1. **YES**
4. Is `--tui` as an explicit flag wanted, or is "no flags → TUI" enough?
    1. Both
5. Quit while a scan runs: block quit, or confirm and detach/cancel?
    1. Detach and run the scan in a background process (fork) on the machine

## Colour output (3.x)

Approved 2026-10-19. `--color auto|always|never`, default `auto`. `auto` follows [NO_COLOR](https://no-color.org/): a non-empty value drops foreground and background colour and leaves bold, dim, and underline. `always` forces colour on. `never` forces it off. `[ui] color` in `~/.acp/preferences.toml` is the default when the flag is omitted. The flag wins. When NO_COLOR is set and colours are off, the terminal UI says so on the status line, in the theme picker, and in the theme editor, including when a theme is chosen or edited.

## Acceptance sketch (for when we implement)

- No flags → TUI; Body empty-state or last nick; Help modal shows real clap help
- Select nick → posts scroll; open post → detail; follow `in_reply_to` → nested
  detail; Esc pops
- Login modal → OTP → session persisted; Logout → login modal
- Start Scanning with nicks → progress → nick counts update; headless CLI flags
  still work as in 1.20
- All existing unit + Gherkin suites stay green; new TUI state tests cover the
  detail stack and Args mapping
