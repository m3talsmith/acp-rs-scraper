# Coming soon: ACP Red Star 3.0, a full client for our forum

Comrades,

**ACP Red Star** (formerly the RS scraper) is on its way to version 3 as a **full Red Star client**: a local, encrypted archive of the forum you'll be able to browse, search, research, and eventually write from, without leaving everything stuck in a browser tab. This is a preview of where 3.0 is headed before it ships.

## What it will be

Red Star is our members-only forum. ACP Red Star is a terminal app that:

- Logs in as you and **archives** posts, replies, mentions, and profiles into a **local encrypted database**
- Gives you a **TUI** (terminal UI) to browse nicks, threads, bookmarks, Explore, Executive Board, and more
- Still supports a full **command line** for scripts, exports, and headless scrapes
- Treats **identity and backups** seriously: password-protected SQLCipher DB, recovery phrase, optional node identity for the decentralization work ahead

The product name is intentional. It is not “just a scraper” anymore. Scraping is how we fill the library; the point is a durable, comrade-controlled copy of our own discussion.

## What’s coming versus the old scraper

**2.x** was mainly: fetch posts into SQLite, search/export, early TUI.

**3.0** will turn that into a client line:

- **Encryption at rest** and a real **account identity** (recovery phrase, shares, pinned id). This is a breaking change from plaintext 2.x DBs
- **Node identity** on your machine (certificates) as the foundation for peer sync, still optional
- **Compose** for New post and Reply in the TUI and CLI, with Cancel-first safety, dry-run, and publish gated until we finish a careful live capture
- A roadmap through **3.x** for attachments, votes/subscribe/edit, live updates, feeds, and **decentralized sync between comrades’ nodes**, without jumping to a 4.0 for that work
- UX we’re finishing in 3.x: clearer menus, Help you can actually navigate, tooltips, and a proper Getting Started path for CLI, interactive terminal, and TUI

Embedded AI assistance is **not** a 3.0 focus; that waits for a later 4.0 line so we do not stall the client itself.

## Why this matters for the ACP and Red Star

Our slogan points forward: **the future belongs to the working class.** A party forum that only lives behind one website is fragile. Members should be able to:

- Keep a **local archive** of education, debate, and organizing discussion
- Work **offline** on what they already have
- Export and share research without begging the browser for it
- Move toward **member-run replication** so knowledge is not a single point of failure

ACP Red Star is being built for that: local-first, encrypted, open source (GPL), and aimed at the same people who already use Red Star, not at turning the forum into a product for outsiders.

## How to try it before launch

**Install** (builds from source):

```text
git clone https://github.com/m3talsmith/acp-rs-scraper.git
cd acp-rs-scraper
scripts/install.sh          # or: scripts/install.sh --user
```

**Three ways you’ll use it:**

1. **TUI**: run with no flags. Unlock with your password, log in under Session, pick nicks, Start Scanning (F6). Press `?` or F1 for Help.
2. **Interactive terminal**: first-run password, encrypt/migrate (`--encrypt-db` from 2.x), backup/restore progress, confirmations like `Publish? [y/N]`.
3. **CLI**: headless, e.g. `acp-red-star --scrape --nicks @yourNick`, `--search`, `--export`, identity flags (`--identity-init`, `--backup`).

**If you’re on 2.x:** copy your DB, then run `--encrypt-db` and write down the recovery phrase when offered. There is no password reset.

**Writing:** you can draft New post / Reply now; live publish stays careful until we finish the guided capture. Attachments land next.

Docs: `docs/INSTALL.md`, `docs/USAGE.md`, and the 3.0 plan under `architecture/PLAN-3.0-CLIENT.md`. Repo: https://github.com/m3talsmith/acp-rs-scraper (product name **ACP Red Star**; binary `acp-red-star`, with `acp-rs-scraper` kept as a compatibility alias through version 3).


## Coming with the launch

When 3.0 launches, we’ll pack a few things so comrades can *see* the client, not only read about it:

- **Elevator pitch**: the short version of why ACP Red Star exists
- **Demo movie**: short screen capture of CLI and TUI usage
- **Cutscene**: infrared-style footage cut with historical communist action scenes, in **two forms**: a normal video, and an **ASCII** version you can play in a terminal (same cut, different skin)

Stickers (graphical + CLI-ethos) and ethos backgrounds will follow as end-of-3.0 promo; the pitch, demo, and cutscene will ride with the 3.0 launch post.

## Ask / help

Bug reports and patches welcome. If you’re testing 3.x ahead of launch, say what machine you’re on and whether you came from a 2.x database. For party process questions, use the usual chapter channels. This post is about the client.

For the working class,
— m3talsmith (and contributors)

---
*Pre-launch post for Red Star. Sign as m3talsmith online. The pitch, demo movie (real CLI+TUI capture), and IR/historical cutscene (video + ASCII) will feature in the 3.0 launch post’s showcase; stickers and backgrounds remain end-of-3.0 promo.*
