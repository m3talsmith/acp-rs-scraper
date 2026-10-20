# ACP Red Star 3.0: Our forum. Your machine. A network no one can switch off.

Comrades,

Today, ACP Red Star 3.0 ships. It's the complete Red Star client. It reads. It writes. It searches, researches, and remembers. It keeps an encrypted copy of our discussion on your own machine, and it shares that copy with comrades across a network that belongs to all of us. It used to be called the ACP RS Scraper. It isn't a scraper anymore. It's the whole forum, in your hands.

## Three ways in. One app.

Some of us live on the command line. Some of us want a real interface. Red Star 3.0 doesn't make you choose.

- **The TUI.** Run it with no flags and you're home: a red header with the star, a nick bar, posts, threads, Explore, Executive Board, all in your terminal.
- **The CLI.** Every action is a flag. Script it, cron it, run it headless on a server.
- **The interactive terminal.** Password prompts, encryption and backup progress screens, a plain `Publish? [y/N]` before anything leaves your machine.

Same archive. Same identity. Three surfaces.

## Your archive. Locked.

Everything Red Star 3.0 stores sits in a **SQLCipher-encrypted database**, protected by your password. Your identity comes with a recovery phrase, recovery shares, and encrypted backups. Lose the laptop and the archive is still sealed. Write down the phrase and you can always restore it.

Your search history is encrypted too. Nothing you look for lives anywhere in plaintext.

## Fill the library exactly the way you want.

Scraping is how the library grows, and now you control every part of it.

- `--limit` sets how deep to go.
- `--profile-feed newest|top` picks which side of a comrade's profile to pull.
- `--order newest|oldest` decides which end you start from.

In the TUI, every view keeps its own order, and every filter carries its own order and limit. Set it once, and it stays the way you left it.

## Search that remembers. Research that goes deep.

Press search. Type. There it is. Every search you run is saved to an encrypted history, with the query *and* the filters, so you can come back to it tomorrow and pick up exactly where you left off. Exports carry that context with them.

Then there's **Research**. Gather posts, files, links, and notes into named research projects. Found a body of work that matters? Add the whole body to a project in one move. Then export it and share it with your study group.

**Bookmarks** keep your favorite posts one keystroke away, and they sync with the site. **Export and import** move your archive as JSON, HTML, or a compressed `.tar.gz`. Your data comes and goes on your terms.

## Write back.

Red Star 3.0 doesn't just read the forum. It talks back.

Compose a **new post** or a **reply** from the TUI or the CLI. Attach files, and the path field **Tab-completes** like your shell does. Cancel is always the default. Dry-run is always available. Nothing publishes until you say yes.

And you can do everything you'd do on the site:

- **Vote**
- **Subscribe**
- **Edit**
- **Delete**
- **Repost**

All from your keyboard. It just works.

## And one more thing…

A forum that lives on one website has a single point of failure. So we removed it.

Red Star 3.0 is a **decentralized network**. Your node syncs with comrades' nodes **peer to peer**, and archives **replicate** across the network. Open **Discover** and you'll find peers, public nodes, relays, and spaces, all from inside the app.

And here's the best part. The **server** and the **relay** ship two ways. They're **separate installs**, their own binaries, ready for a box in a closet or a VPS. And they're **embedded right inside the app**, so any comrade can seed a node fast without installing anything else. Every install makes the network stronger.

Our discussion no longer depends on any one machine. It lives everywhere we do.

## Designed to be understood.

Power means nothing if people can't find it. So we went through every corner of the interface.

- A **categorized menu bar**: **View, Search, Compose, Library, Research, Sync, Discover, Account, Debug**. You know where everything is before you look.
- **Hover tooltips on everything.** Every button, every menu, every option. And every tooltip links to **its own Help topic**. Not a generic page. The exact topic for the exact thing you're pointing at.
- **Help is topic-based and searchable.** Type what you want to know and go straight there.
- A **guided walkthrough** for new comrades covers all three surfaces, step by step, from install to first post.

And there's an **About** menu with three segments: **Credits**, for the comrades who built this; **Why this is necessary**, the reason it exists; and the **ACP platform**, the official Party Program, readable right in the app, with a **Join** link to https://acp.us/join.

## Make it yours.

Red Star 3.0 comes with **themes**, including **CCP**, a vermilion-and-gold poster palette that looks stunning in a terminal. Want your own? Open the **theme editor** and watch every change **hot-reload** live while you work.

Then the little things that make it a joy to use every day:

- An **action-history bar** that slides out on the right, so you always know what just happened.
- A **masonry layout** that fills your screen with posts, wall-to-wall.
- **Fullscreen read**, for when one post deserves your full attention.

## Built to be built on.

Red Star 3.0 isn't only an app. It's a foundation. The code is organized into **Rust crates** that anyone can build on: the client library (`acp_red_star`) and the shared node core (`acp_red_star_node_core`) for identity, canonical CBOR, and sealed secrets. Write a tool, a bot, a dashboard, a new client. It's GPL, and it's yours.

## Launch showcase

Don't just read about it. Watch it.

**The elevator pitch**

> *[PLACEHOLDER: elevator pitch]*

**The demo movie.** The real CLI and TUI, on a real archive, in the CCP theme.

> *[PLACEHOLDER: embed `redstar-v3-demo-dramatic.mp4`]*

**The cutscene.** Infrared footage cut with historical communist action scenes. Two forms, same cut:

> *[PLACEHOLDER: IR/historical cutscene, normal video]*
>
> *[PLACEHOLDER: IR/historical cutscene, ASCII version, plays right in your terminal]*

## Get started in minutes.

**Download.** Prebuilt binaries are on GitHub Releases:

> https://github.com/m3talsmith/acp-rs-scraper/releases *(placeholder URL)*

**Or build it yourself:**

```text
git clone https://github.com/m3talsmith/acp-rs-scraper.git
cd acp-rs-scraper
cargo build --release        # or: scripts/install.sh  (--user for a per-user install)
```

**First run:**

1. Launch it with no flags. Set your password.
2. Write down your recovery phrase. There is no password reset, by design.
3. Open **Account** and log in to Red Star.
4. Pick your nicks and start scanning (**F6**).
5. Press `?` or **F1** any time, or follow the walkthrough under Help › Getting started.

Coming from 2.x? Copy your database, run `--encrypt-db`, and save the recovery phrase when it's offered. Your old archive comes with you, now encrypted.

## Why this matters

Our slogan points forward: **the future belongs to the working class.** The working class needs its own memory.

Years of education, debate, and organizing live on Red Star. That knowledge shouldn't depend on one website, one server, or one browser tab. With Red Star 3.0, every comrade can hold an archive they own, encrypted on their own machine, readable offline, shareable on their own terms. And because the network is decentralized, every comrade who runs a node makes the whole thing harder to lose.

It's local-first, encrypted, resilient, and open source. It's built for the people who already call Red Star home, not for outsiders.

Not a member of the American Communist Party yet? Read the Program in the About menu, and then join us: **https://acp.us/join**

## What's next

One more thing we're excited about: **4.0** brings embedded local AI, running on your own machine, alongside your archive. But that's a story for another day.

Today, go download Red Star 3.0. Open it up. Make it yours.

For the working class,
— m3talsmith (and contributors)
