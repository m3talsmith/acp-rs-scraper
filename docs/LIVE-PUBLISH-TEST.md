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
- A text-only dry-run has no file name inside the validate and save frames. An attachment dry-run is a later step. It prints an `allow_upload` plan and still does not connect.
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

## Attachments

Do this only after the text-only post above has worked, and only with files you are willing to put on the site. Use a throwaway sentence so you can find the post later. Do not pass `--yes` on the first try of each file. A second confirm inside 30 seconds waits. That wait is expected.

The client uploads the site copy, then sends validate and save once. It does not retry a chunk, and it does not send the post again if the upload fails. If the error says the post was not saved, the bytes did not become a post. If the error says to check the site, look before you post again.

### Dry-run with a small image

Pick a small JPEG or PNG, well under a megabyte.

```bash
./target/release/acprs --new-post "upload check image" --body "small image from the client" --attachment /path/to/small.jpg --dry-run
```

Check the printout:

- The first line says `dry-run: no connection opened`.
- An `upload plan` names `media_attachments`, `ref=phx:<page>`, the file name, `image/jpeg` or `image/png`, the size, and a chunk count. `chunk_size 64000` is the captured default. A live reply can change it.
- The validate and save frames are still there, and they do not contain the file bytes.
- There is no session cookie, no one-time code, no `_csrf_token` value, and no `SFMyNTY` blob.

### One small image

```bash
./target/release/acprs --new-post "upload check image" --body "small image from the client" --attachment /path/to/small.jpg
```

Answer `y` only when you mean to post. Stderr prints `uploading small.jpg N%` as the integer percent changes, then the command prints:

```text
published POST_ID
https://acp.us/rs/post/...
```

In the full-screen client, `n`, attach the same file, move from Cancel to Publish, and confirm. The status line shows the same `uploading` lines, then `publishing…`, then a Published box.

Open the URL. The image should be on the post. If the site rejected the file, the command names it and says nothing was sent or nothing was retried. Do not run it again until you have read that line.

### Optional small PDF

A PDF uses the separate file input (`file_attachments`, accept `.pdf`). It counts toward the same 4 as images and videos.

```bash
./target/release/acprs --new-post "upload check pdf" --body "small pdf from the client" --attachment /path/to/notes.pdf --dry-run
```

The plan should name `file_attachments` and `application/pdf`. Then, when you mean to post:

```bash
./target/release/acprs --new-post "upload check pdf" --body "small pdf from the client" --attachment /path/to/notes.pdf
```

Open the URL and confirm the PDF is attached. This step is optional. Skip it if you only wanted the image.

### Then a video

Use a short MP4 you can afford to upload. The progress line may sit at `0%` for a while, because percent is an integer and the chunks are 64000 bytes until the server says otherwise.

```bash
./target/release/acprs --new-post "upload check video" --body "short video from the client" --attachment /path/to/clip.mp4 --dry-run
./target/release/acprs --new-post "upload check video" --body "short video from the client" --attachment /path/to/clip.mp4
```

Confirm the video plays or is listed on the post. A reply with an attachment is the same flags with `--reply THREAD` instead of `--new-post`.

Audio and other documents are kept locally and named as not uploaded. They should not appear on the site. `.tif` is not renamed to `.tiff`. If the form's accept list rejects a name, the command stops before save.

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

Say whether you used the command line or the compose box, whether the attempt was text, an image, a PDF, or a video, and whether the post was still visible before you deleted it. Do not paste an upload token or a chunk of the file.
