# Interaction capture

A vote (`1`, `0`, `-1`), a site bookmark (`state` `1`), a subscribe (`state` `1`), a delete (`delete-post`), a top-level post edit, and a quote repost are sent. `--dry-run` prints the frames and does not connect.

Still not sent, and the missing frame is named:

- Unsubscribe: a subscribe hook with state "0".
- Clearing a site bookmark from this command: a bookmark hook with state "0". `--sync-bookmarks` still pushes a removal with state "0". That frame was not in this capture.
- A plain repost: a save on `/rs/post/new?repost=POST` with an empty form[body_input].
- Share is not a write. The site's Share post control is client-side only. No frame is treated as share.

The site does not support changing attachments on an edit. `--attachment` with `--edit` is refused before a connection.

## Vote

Page: `https://acp.us/rs/post/POST`. The hook is `vote` with `id` and `state` `1`, `0`, or `-1`. The reply shows `score`, `vote`, and `upvote_percentage`.

## Subscribe

Page: `https://acp.us/rs/post/THREAD`. State `1` is sent. State "0" is not.

## Bookmark

The hook is `bookmark` with `id` and state `1`. The reply shows `bookmarked` and `bookmarks_count`. State "0" is not sent by `--site-bookmark`.

## Delete

`delete-post` with `id`. Cancel sends no frame. Success is `live_redirect` to `/rs`. The local copy stays, with a tombstone.

## Edit

A reply edit keeps a leading `postRef` from the mounted editor. The dry-run says the quote is kept and is not shown. A top-level edit sends `form[title]` and `form[body_input]` after the redirect join to `https://acp.us/rs/post/POST/edit`. A field that was not typed is read from the editor after confirm. The one-hour window still refuses before a connection.

## Repost

A quote repost opens `/rs/post/new?repost=POST` and saves `form[quoted_post_id]`, a title, and a short comment. An empty comment is not sent.
