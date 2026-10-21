# Upload capture (2026-10-21)

A confirmed new post or reply uploads the site copy, then sends the existing validate and save frames once. This note is the protocol those frames follow. The bytes, cookies, and signed values from the capture are not in the repository. `tests/fixtures/site/upload.json` is a minimized stand-in with invented names, refs, and tokens.

The capture was one new post on the live composer: a JPEG and an MP4 on `media_attachments`. Chrome DevTools recorded the LiveView socket (`wss://acp.us/live/websocket`). No HTTP PUT and no direct upload to object storage appeared. File bytes moved as binary websocket frames.

The screen recording of the same action is only a UI map: the paperclip opens a file picker, thumbnails show as soon as the files are chosen, and the thin progress bar plus the disabled "Posting..." button appear after Post is clicked. The page then redirects and shows a success toast.

## Order

1. Join the compose page (`phx_join` on `lv:<id>`), the same join text publish already uses.
2. Read `data-phx-upload-ref`, `name`, and `accept` from the joined render. The initial HTML shell does not contain the form. A preview node may repeat the ref; the file input is the one with `name`.
3. For each input that has site files, one top-level `allow_upload` (not wrapped in `event`).
4. For each entry, join `lvu:<entry ref>` with the token from the reply, send binary `chunk` frames, and send `progress` on the page channel after each acknowledged chunk.
5. After every file has reported progress 100, send the existing validate frame, then the existing save frame, once. Do not send a validate after save. Do not retry.

Files on one input share one `allow_upload`. This client uploads files one after another. The browser in the capture overlapped the two files. Either order is a valid client.

Entry refs start at `"0"`. The capture's successful files were `"1"` and `"2"` only because an earlier file, ref `"0"`, was cancelled during composition. The server keys tokens by the refs we send. Refs are unique across the post so the `lvu` topics do not collide.

## `allow_upload`

Page channel, event name `allow_upload`:

```json
{
  "ref": "phx-UPLOADREF",
  "entries": [
    {
      "last_modified": 1700000000000,
      "name": "photo.jpg",
      "relative_path": "",
      "size": 1200,
      "type": "image/jpeg",
      "ref": "0"
    }
  ],
  "cid": 1
}
```

`ref` is the input's `data-phx-upload-ref`. It is not invented. Dry-run has not joined, so it prints `phx:<page>`. `relative_path` is `""`. `last_modified` is the file mtime in milliseconds, or 0. `type` is the MIME type. `cid` is the compose form's `phx-target`.

The reply is `phx_reply` status `ok`. The useful keys under `response` are:

| key | captured value |
|---|---|
| `config.chunk_size` | 64000 |
| `config.chunk_timeout` | 10000 (milliseconds) |
| `config.max_entries` | 4 |
| `config.max_file_size` | 250000000 |
| `errors` | `{}` |
| `entries` | map of entry ref to the join token |

The token is what the `lvu` join sends. It is a signed value. This client does not log it. A non-empty `errors` value, a status other than `ok`, or a missing token stops the publish before any chunk and before save. The message names the reason and says nothing was sent.

`config` is per input. A later reply can change `chunk_size`. Dry-run prints the captured defaults and says the server can change them. A live upload uses the reply. A `chunk_size` of 0, or one this client will not allocate, stops before any chunk.

If the reply allows fewer files or a smaller file than the one we offered, the publish stops and names the file. Nothing is dropped to make the rest fit.

## `lvu` join and chunks

```json
["10", "10", "lvu:0", "phx_join", {"token": "synthetic-token-entry-0"}]
```

`join_ref` and the message ref are the same string. They are not the page join ref `"1"`. The reply is status `ok` with an empty response.

A chunk is a Phoenix V2 binary push, not a JSON frame:

| offset | meaning |
|---|---|
| 0 | kind `0` (push) |
| 1 | join-ref length |
| 2 | message-ref length |
| 3 | topic length |
| 4 | event length |
| then | those strings, then the raw file bytes |

The event is `chunk`. The topic is `lvu:<entry ref>`. A full captured chunk is 21 header bytes plus 64000 payload bytes when the refs are three digits and the topic is `lvu:1`. The last chunk of a file is shorter. Each chunk waits for a text `phx_reply` on that message ref, bounded by `chunk_timeout`. A timeout or a non-ok reply leaves the `lvu` topics and does not save. The chunk is not sent again.

The debug log line is `binary <topic> chunk <n> bytes`. The file bytes are not logged.

## `progress`, then save

After each acknowledged chunk the client sends `progress` on the page channel:

```json
{"event": null, "ref": "phx-UPLOADREF", "entry_ref": "0", "progress": 6, "cid": 1}
```

`progress` is the integer `floor(sent * 100 / size)`, and 100 when the file has been sent. The first 64000-byte chunk of a 1,019,521-byte image is 6. The first chunk of a much larger video is 0. Both files in the capture ended at 100. The command line prints `uploading {name} {pct}%` when that integer changes. The compose box puts the same line on the status line.

`event` is null. Save is a separate frame, sent only after progress 100 has been acknowledged. The save frame is the text publish frame: new posts are `form[title]` and `form[body_input]` with no `_target` and no `uploads` key. Replies keep `meta._target` on validate and an empty `meta` on save. A keystroke validate in the capture did carry an `uploads` map. This client does not send those keystroke frames. The capture's later body validates used `uploads: {}` and the upload still completed, so an empty `uploads` object does not cancel entries.

The post id still comes from `thread:insert` or `live_redirect`. The signed flash is not shown. A rejection after the bytes were sent says nothing was retried and asks you to check the site. The client does not send the post again.

On a failure before save, the client sends `phx_leave` on the `lvu` topics it joined and closes the socket. `cancel-media-upload` was a click during composition in the capture. It is not part of the happy path, and this client does not send it.

## What is uploaded

The site copy is at most 4 files. Images and videos use `media_attachments`. A PDF uses `file_attachments`. The composer render already shows that split: media `accept` is `.avif,.gif,.jpg,.jpeg,.png,.tiff,.webp,.mkv,.mov,.mp4,.webm`, and the PDF input's `accept` is `.pdf`. Michael confirmed the site accepts PDFs. The PDF bytes themselves were not in this capture. The same `allow_upload` / `lvu` / `progress` sequence is used on whichever input's accept list matches the file.

The live form wins over the list above. An empty `accept` does not refuse the file; the server's `errors` map does. `.tif` is not rewritten to `.tiff`, and `.jpeg` is not rewritten to `.jpg`. If the form's list does not contain the name, the publish stops before save and names the file.

Audio and other documents stay on this machine. The success text and the dry-run list them as not uploaded. They are not omitted silently. An empty site file is refused before a connection.

The 30 second gap, `Publish? [y/N]`, and Cancel as the default button are unchanged. `--dry-run` opens no socket. It prints the `allow_upload` bodies, the entry refs, and the chunk counts. Vote, subscribe, delete, edit, and repost stay gated.

## Open questions

- The PDF input's `max_entries` and `max_file_size` were not in the capture. Only the media `allow_upload` reply was. This client sends a PDF with the same frames and then obeys whatever that reply says. It also counts a PDF toward the same client cap of 4 as images and videos. If the site allows 4 media files and more PDFs on top, this client will not offer that until a capture shows it.
- No `too_large`, chunk error, or `chunk_timeout` reply was captured. Stopping on a non-ok reply, a non-empty `errors` value, or a timeout is this client's rule. The `errors` value is read as an entry ref mapped to a list of reasons. Any other non-empty shape still stops the publish and shows a redacted excerpt.
- `progress` with `"event": "save"` (submit while bytes are still moving) was not observed. This client finishes the upload, then sends save.
- A validate that lists the uploads before `allow_upload` was not required by the frames that actually carried the bytes. If a future capture shows the server dropping entries without that validate, the order here has to change.
- The browser overlapped the two files. This client does not.
- A file the server would accept, but that this client's extension list does not mark for the site (anything other than an image, a video, or a PDF), stays local and is named. The live `accept` attribute is not consulted for those files, because they are classified before the page is joined.
