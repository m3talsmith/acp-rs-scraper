# Getting started

ACP Red Star is a local client for the Red Star forum. The short command is `acprs`. `acp-red-star` is the same program. `acp-rs-scraper` still runs, through version 3, and prints one deprecation line.

This walkthrough is the Getting started topic in Help (`?` or F1, then search `getting`). It has three surfaces.

- **CLI** is a headless flag. The program does the work and exits.
- **Terminal** is a prompt or a progress line when the full screen is not open: the database password, the recovery phrase, `Publish? [y/N]`, and scrape progress on stderr.
- **TUI** is the full-screen client. It is what you get when you run `acprs` with no flags.

`architecture/SPECS.md` and `architecture/PLAN-2.0-TUI.md` still describe the 2.0 design. The client has moved on. This walkthrough follows the app. Hover a control for a short explanation. The `?` on that tooltip opens the Help topic for that action.

A text post or reply is sent. Images, videos, and PDFs on that post are uploaded first, at most 4. Other files stay on this machine and are named. A reply edit sends validate, then save, once, and keeps a leading quote. A vote, a site bookmark, a subscribe, a delete, a top-level post edit, and a quote repost are sent. Unsubscribe and clearing a site bookmark are not sent. The site does not allow a plain repost. `--dry-run` prints the frames and the upload plan and does not connect. The site does not support changing attachments on an edit. The frames still missing are `architecture/INTERACTIONS-CAPTURE.md`.

## Install

From a clone:

```
scripts/install.sh
```

Or `cargo install --path .`. That installs `acprs`, `acp-red-star`, and `acp-rs-scraper`. `acprs --version` prints `acp-red-star` and the version.

## Encrypt and identity

A database this program creates is encrypted. The first start on a terminal asks for a password. There is no reset. The password is not the master key.

```
acprs --identity-init
```

That shows a 24-word recovery phrase once. You type it again to confirm. It can then split the phrase into 5 shares. Any 3 of those 5 rebuild the phrase. Write the words and the shares on paper. `--show-phrase` and `--shares-print` ask again later. On a terminal those prompts are the program waiting, not the TUI.

`--encrypt-db` encrypts an older plaintext database after it writes a backup.

## Open the TUI

```
acprs
```

The screen is the header, the menu bar, the nick list, the body, and the status line. `?` in the top corner, `?`, and F1 open Help. About, beside Help, opens one window on Credits, Why this is necessary, or ACP platform.

## Log in

Browsing the local database needs no login. Start Scanning does. Account › Log in asks for the email, then the one-time code. The same steps exist as `acprs` waiting on the terminal when a headless command needs a session. The session is saved in `~/.acp/session.json`.

## Scrape

Pick nicks, then Start Scanning (F6), or:

```
acprs --scrape --nick alice --no-mentions
```

Progress is on the status line in the TUI, and on stderr when you are not in the TUI. `--limit` and `--order` keep part of a profile. Omit `--limit` to read the whole profile.

Whole site, on Scrape or on Sync, reads every reachable page:

```
acprs --scrape-all
acprs --sync --scrape-all
```

`acprs --scrape-all` resumes if it stopped. The sync form is the incremental update, and rules run on the posts it stored. `--scrape-budget 600` stops after ten minutes and leaves the rest for the next run.

## Browse and search

Click a nick, or press Enter, to read posts. `b` bookmarks the open post. `/` filters the nick list. Search on the menu reads the local database. Search the site uses the same query against the forum. `Shift-S` opens the full search. `<` goes back.

## Export

Library › Export writes JSON or HTML for the selected nicks, the current view, everything, or the basket. The same flags work headless: `--export`, `--format`.

## Compose, dry-run

`n` starts a new post. On an open post, `r` replies and `Q` quotes. Cancel is the default button. Publish uploads the site copy (images, videos, and PDFs), then sends the text. The status line shows `uploading name N%`, then `publishing…`.

```
acprs --new-post "Title" --body "hello" --dry-run
```

On a terminal that is not the TUI, a real publish asks `Publish? [y/N]`. Answering yes sends validate, then save, once, and prints the new post id and URL. `--dry-run` does not ask, and it does not connect. `+` and Post menu › Subscribe, Delete, Edit, and Repost ask before they prepare a frame. Cancel is the default. Delete and edit also ask you to type yes. A confirm still sends nothing. The first live check is [LIVE-PUBLISH-TEST.md](LIVE-PUBLISH-TEST.md).

## Help and About

Help is topics: Getting started, Browse, Compose, Research, Sync, Discover, Account, Keys, and the menu topics. Type to filter. Left and right move between topics. Every button, menu entry, and card icon has a tooltip, and the tooltip's `?` line opens that action's topic.

About has three sections and no others. ACP platform is the party program adopted October 12, 2024, with headings and a scroll. The link to join is at the top and the bottom of that section only.
