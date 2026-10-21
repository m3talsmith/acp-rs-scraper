# Interaction capture (2026-10-21 audit)

Plan **3.37.0** checked vote, subscribe, delete, edit, and repost against the frames already in this repository. None of them have a proven websocket frame. All five stay behind `CAPTURE_BLOCK`. A confirm still sends nothing. `--dry-run` prints the stub and does not connect.

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

## Edit — gated

Page: within one hour of a post you own, click Edit. The survey route is `https://acp.us/rs/post/POST/edit`. If the click opens a different URL, record that URL.

Clicks:

1. Change only the title, then save. The check post's title typo was already corrected on the website, so this is a further title-only change, or the HAR of that website edit if it was recorded. Do not treat the typo fix as this frame unless the websocket for it was saved.
2. On a second pass, change only the body, then save.
3. Edit: replace attachment. On the check post that already has the PNG, click Edit, remove the existing image, upload a different small image, then save.
4. If you can open a post older than one hour, click Edit and record the refusal. Do not post a new one just to wait out the hour unless you mean to.

Record the LiveView join (form id, `phx-submit`, `phx-change`, and cid), the validate payload, and the save payload. The save has to show which keys are omitted when a field did not change. Also record the reply (`thread:update-post` or whatever the site actually pushes).

For the replace-attachment click, record two more things and do not summarize them away. First, the frame that removes the image already on the post (the event name, the payload, and the reply). Second, the new file's `allow_upload`, the `lvu` join, the binary `chunk` frames, and `progress`, the same channel a new post uses. This client does not build the form and does not send a removal or an upload on edit. When those frames are in the tree, an edit that replaces an attachment reuses the 3.36.0 `allow_upload` / `lvu` path for the new file and sends the removal frame the capture shows. Until then the edit stays gated. An edit older than one hour is refused before the confirm.

## Repost — gated

Page: `https://acp.us/rs/post/POST` for a post you can repost.

Clicks:

1. Click Repost. If the comment box can be left empty, save once that way.
2. Click Repost again (or use another post) and type a short quote, then save.

Record the URL the click opens. The survey route is `/rs/post/new?repost=POST`. Record the join, how `quoted_post` is encoded, whether a title is required, the validate and save fields, and the new post id in the reply. This client names the survey route and does not build the form. An optional quote is not sent until that save frame shows the field.

## What a useful HAR contains

Chrome DevTools, Network, the `wss://acp.us/live/websocket` connection, preserve log. One HAR may hold all five if each click is a separate moment you can point at. Redact cookies, the CSRF token, signed flashes (`SFMyNTY`), and upload tokens. Leave the event name, the payload keys, and the reply keys. Do not commit the raw file.
