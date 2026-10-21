# First live text publish

This is a manual checklist for Michael and the maintainer. Run it together, by hand, from a clone on a machine you control. Do not run it in CI. Do not run it from an agent VM. Do not point it at anyone else's `~/.acp`.

One short post or one short reply is enough. The client sends validate, then save, once. It does not retry. If the reply is unclear, look at the site before posting again.

## Before you start

1. Clone the repository yourself and check out the branch you intend to try. Build it there.

   ```bash
   git clone https://github.com/m3talsmith/acp-rs-scraper
   cd acp-rs-scraper
   git checkout <the branch you agreed on>
   cargo build --release
   ./target/release/acprs --version
   ```

   `acp-red-star` is the same program. Use the binary you just built, not a copy installed from an older release.

2. Log in as the account that will own the post. In the full-screen client, Account › Log in asks for the email and then the one-time code. A headless command that needs a session asks the same questions on the terminal. The session is saved in your own `~/.acp/session.json`.

   Do not paste the code, the session file, or a cookie into chat, a log, or this checklist.

3. Writes are on unless `ui.writes = false` in `~/.acp/preferences.toml` or `ACP_WRITES=off`. Leave them on for this check.

## Dry-run first

A dry-run opens no connection and does not post. Run one before the real send. Pick a new post or a reply, not both.

New post:

```bash
./target/release/acprs --new-post "text publish check" --body "short note from the client" --dry-run
```

Reply, using a thread id you can see on the site:

```bash
./target/release/acprs --reply THREAD --body "short reply from the client" --dry-run
```

Check the printed frames:

- The first line says `dry-run: no connection opened`.
- A reply shows `form[thread_id]` and `form[body_input]`, then `"event": "validate"` and `"event": "save"`.
- A new post shows `form[title]` and `form[body_input]`, then the same two events.
- There is no third event after save.
- No file name is inside the frames. If you passed `--attachment`, remove it. A post that still has files is refused and nothing is sent.
- The printout has no session cookie, no one-time code, and no `_csrf_token` value.

## One real post

Use the same text you just reviewed. Do not pass `--yes` on this first try. A terminal asks `Publish? [y/N]`. Answer `y` only when you mean to post. Anything else sends nothing.

```bash
./target/release/acprs --new-post "text publish check" --body "short note from the client"
```

Or:

```bash
./target/release/acprs --reply THREAD --body "short reply from the client"
```

In the full-screen client the same check is `n` for a new post, or `r` on an open post for a reply. Focus starts on Cancel. Move to Publish and confirm. The status line says `publishing…`, then a Published box names the id and the URL.

On success the command prints two lines:

```text
published POST_ID
https://acp.us/rs/post/...
```

A reply URL is `/rs/post/THREAD/POST_ID`. Open that URL on the site and confirm the title or the body is the text you typed, on the account you logged in with.

A second confirm inside 30 seconds waits. That wait is expected. Do not send a second post to get around it.

## If the site refuses it

The command prints the reason and says nothing was retried. Read that reason. Do not run the command again until you know the first attempt did not land. A save that was sent, with no id in the reply, says to check the site before posting again. Search your profile for the exact sentence you used. If it is there, do not post it again.

## Delete it on the website

This client does not delete. `--delete` and Post menu › Delete prepare the hook and do not send it.

To remove the check post, open it in a browser on acp.us while you are logged in, and use the site's own delete control. Confirm on the site. Reload the URL and confirm the post is gone or tombstoned.

## What to share afterwards

Share the dry-run text, the two success lines (id and URL), and, if it failed, the error line. A debug log (`--debug`) redacts cookies, the CSRF token, and a signed flash. Still do not paste:

- `~/.acp/session.json`
- the one-time code
- a cookie or `_csrf_token` value
- a signed flash (`SFMyNTY…`)
- anyone else's home directory

Say whether you used the command line or the compose box, and whether the post was still visible before you deleted it.
