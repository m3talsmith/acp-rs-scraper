# Interaction capture (2026-10-21 audit, reply edit 2026-10-21)

Vote, subscribe, delete, and repost still have no proven websocket frame. They stay behind `CAPTURE_BLOCK`. A confirm still sends nothing. `--dry-run` prints the stub and does not connect.

A reply-body edit is captured and sent. The open is `phx_leave`, then `phx_join` with `redirect` `/rs/post/{reply}/edit` (`sticky` false, `_mounts` 0, no `_track_static`). The mounted form is `js-postForm-form`, `method="put"`, `data-mode="reply"`, `phx-submit="save"`, `phx-change="validate"`, `phx-target="1"`, and the only field is `form[body_input]`. The client sends one validate (`_target` `form[body_input]`, empty `uploads`, cid 1) and one save (empty `meta`, no `uploads` key). It does not send the keystroke validates, the trailing validate that got `unmatched topic`, or `mention-query`, `view-post`, `refresh-posts`, and `lv:clear-flash`. Success is the `live_redirect` to `/rs/post/{thread}/{reply}`. The masked shape is `tests/fixtures/site/edit.json`. A `postRef` already on the reply is not copied: dry-run cannot see it, and the sent body matches the printed body. A top-level post edit, attachment replace, and the over-one-hour refusal frame are still needed. Those stay behind `CAPTURE_BLOCK`.

What exists today is a 2026-10-18 JavaScript survey (plan rows 26–31) plus the stubs in `tests/fixtures/site/{vote,subscribe,delete,edit,repost}.json`. Each stub says `"capture_status": "missing"`. The documented hook names (`vote`, `subscribe`, `delete-post`) and the routes (`/rs/post/{id}/edit`, `/rs/post/new?repost={id}`) are not a live `phx` frame. The reply join in `tests/fixtures/site/reply_join.json` shows post props (`score`, `vote`, `upvote_percentage`, `subscribed`, and `actions` for delete, edit, and repost). Those props are the render. They are not the click.

Do not invent the payload from the survey. Record the clicks below in Chrome on the live site, export a HAR, redact it, and do not commit the raw HAR. The client steps in `docs/LIVE-PUBLISH-TEST.md` stay blocked until those frames are in the tree.

## Vote — gated

Page: a thread you may vote on, `https://acp.us/rs/post/POST`. Use the control on that post. A reply uses the same page, on that reply.

Clicks, in order, on that one post:

1. Click the upvote control.
2. Click that same control again, so the vote clears.
3. Click the downvote control.
4. If a second click on downvote clears it, click it again.

Record every websocket event and its `phx_reply`. The frame has to show the event name, whether the payload is a hook, which component it targets, and the value (`id` and `state` for `1`, `0`, and `-1`). The reply has to show `score`, `vote`, and `upvote_percentage` after each click. A rejection, if one is cheap to cause, belongs in the same capture. The client will not toggle or display a server state until that reply exists. `+` prepares an upvote and `-` prepares a downvote. Clear is `--vote --vote-state 0`.

## Subscribe — gated

Page: the thread, `https://acp.us/rs/post/THREAD`. Not a profile.

Clicks:

1. Click the subscribe control on that thread.
2. Click it again so the subscription clears.

Record both frames and both replies. Note whether `id` is the thread id or a reply id. The reply has to show `subscribed` true, then false.

## Delete — gated

Page: `https://acp.us/rs/post/POST` for a post you own. `actions.delete` is true on your own post. Use a throwaway check post.

Clicks:

1. Open the site's own post menu and click Delete.
2. On the site's confirm dialog, choose cancel once. Record whether that click sends a frame.
3. On a post you are willing to remove, open Delete again and confirm it.

Record the event, the dialog text, the `phx_reply`, and any `feed:delete` or tombstone diff. Note whether the body stays readable. This client's Delete still asks you to type yes and then does not send.

## Edit — reply body is sent; the rest stay gated

A reply-body edit is in the client (crate 3.33.0). `--edit REPLY --body TEXT` sends validate, then save, once, after `Edit? [y/N]` and `Type yes`. `--dry-run` prints those frames and does not connect. The one-hour window still refuses before a connection.

Still needed, and still behind `CAPTURE_BLOCK`:

1. Change only the title of a top-level post, then save. The survey route is `https://acp.us/rs/post/POST/edit`. The reply capture opened `/rs/post/{reply}/edit` and had no `form[title]`. A page with a title field is refused and nothing is sent.
2. On a second pass, change only the body of that top-level post, then save.
3. Edit: replace attachment. On the check post that already has the PNG, click Edit, remove the existing image, upload a different small image, then save. Record the removal frame and the new file's `allow_upload`, the `lvu` join, the binary `chunk` frames, and `progress`. Until those frames are in the tree, `--attachment` with `--edit` is refused.
4. If you can open a post older than one hour, click Edit and record the refusal. The client already refuses from the timestamp. The site's own refusal frame is still useful.

The reply capture kept a leading `postRef` (a quote of another post). This client does not copy that node, because dry-run cannot see the mounted document and the sent bytes have to match the print. Say if that quote should be kept automatically.

## Repost — gated

Page: `https://acp.us/rs/post/POST` for a post you can repost.

Clicks:

1. Click Repost. If the comment box can be left empty, save once that way.
2. Click Repost again (or use another post) and type a short quote, then save.

Record the URL the click opens. The survey route is `/rs/post/new?repost=POST`. Record the join, how `quoted_post` is encoded, whether a title is required, the validate and save fields, and the new post id in the reply. This client names the survey route and does not build the form. An optional quote is not sent until that save frame shows the field.

## What a useful HAR contains

Chrome DevTools, Network, the `wss://acp.us/live/websocket` connection, preserve log. One HAR may hold all five if each click is a separate moment you can point at. Redact cookies, the CSRF token, signed flashes (`SFMyNTY`), and upload tokens. Leave the event name, the payload keys, and the reply keys. Do not commit the raw file.
