# Spaces

Read-only list: crate **3.42.0**. Joining a room is not built. Nothing in this
file is implemented past the list, and none of it runs until Michael records
the frames described below and they are redacted on his machine.

The list reads `/rs/spaces` (a feed of space posts) and the `space` key on a
post, plus `forum/SpaceBar` when that bar carries a state or a recording link.
View › Spaces and `--spaces` open it. Sync, and `--scrape-all`, store it in
the encrypted database and stop early once the first page is already known.
A second `--spaces` logs `all known`.

What the list shows:

- **LIVE** when the page says the room is live.
- **scheduled** or **ended**, or whatever state string the page exposes.
- The scheduled time and the end time when the page has them.
- A recording link when the page has an ordinary `http` or `https` URL.

## The LiveKit token is a secret

A LiveKit URL (`ws://`, `wss://`) and a LiveKit token are secrets. The client
does not request one. If a page embeds one, it is dropped before a row is
stored, printed, or logged. It is not a database column. It is not written to
notices, action history, or a crash line. A recording link that is itself a
LiveKit URL is dropped with the token.

The 3.42.0 fixtures are a stand-in for the read-only page shape. They are not
a capture of acp.us.

## Keys

There is no new global Ctrl chord and no new mode hotkey. The action id is
`view.spaces`. It opens from View › Spaces and from `--spaces`, the same way
Explore opens from the View menu. `architecture/PLAN-MODES-KEYMAP.md` is not
on this branch. When that plan lands, add `view.spaces` to the view actions.
Do not hardcode a new key for it.

## Joining a room (gated)

Every step below waits on the capture. The client must not call these events
before that, including from a test that talks to acp.us.

| Action | What it would do | First shape |
|---|---|---|
| Join | `space-join` for one room | Listen-only. No microphone until the person asks. |
| Leave | `space-leave` | Always available once joined. Leaving drops the token from memory. |
| Raise hand | `space-raise-hand` | After listen-only works. |
| Lower hand | `space-lower-hand` | The same control, the other way. |
| React | `space-react` | The reaction ids come from the capture, not from a guess. |
| Recording | `space-recording` | Separate from the read-only recording link. |
| Start a space | `start-space` | Host only, after the throwaway-space capture. |
| Cancel a space | `cancel-space` | Host only. Cancel is the default if a confirm is required. |
| Host tools | `space-mute`, `space-promote`, `space-demote`, `space-end`, and whatever else the host bar sends | Host only. Each one is named from the capture. |

`/rs/spaces/{id}/edit` stays closed until its form is in the same capture.

Listen-only is the first join. Raising a hand, reacting, publishing audio,
and every host tool come after that works, each behind its own confirm where
the site asks for one.

## Audio and video from a terminal

A terminal client would join LiveKit with a Rust SDK. Two options to decide
after the capture, not before:

- The `livekit` crate (and `livekit-api` for the room token the server already
  minted). It publishes and subscribes to tracks. Audio-only means subscribe
  to audio and do not publish a video track.
- A lower-level WebRTC stack, only if the LiveKit crate cannot run headless.
  That is a larger dependency and should not be the first try.

Device selection is an input device and an output device, chosen in the
terminal before publish. The default output plays the room. The default input
stays closed until the person leaves listen-only. Video in a terminal is a
later question: a TUI cannot show a camera the way a browser can, so the first
join is audio-only even if the room has video.

The token the server returns for that join is the same secret as above. It
is held in memory for the connection and wiped on leave. It is never logged,
never stored, and never printed in `--debug`.

## Capture Michael would do

Do this on his machine. Do not commit the HAR. He saves it, and the redaction
happens on his machine before anyone else sees it.

1. Open DevTools first, with the network log recording.
2. Reload the page so the LiveView websocket is in the log.
3. Join a space, raise a hand, react, and leave.
4. Host a throwaway space. Start it, cancel it, mute, promote, and demote.
5. Save the HAR.

Until that HAR exists, the client only lists rooms.

## Open questions

1. Is the first join listen-only, or should the microphone open with the room?
2. Is the terminal client audio-only, or is video in scope later?
3. Which stack: the `livekit` crate, or a lower-level WebRTC crate?
4. How should the terminal list and pick an input device and an output device?
5. Do recordings play only in an external player?
6. Does a live badge refresh from a websocket, or only on the next sync or `--spaces`?
7. Does an ended space stay in the list?
8. The fixture's `state`, `live`, `recording_url`, and `forum/SpaceBar` props are a stand-in. Which names does the live page use?
9. Which events should the HAR include beyond join, leave, raise hand, lower hand, react, recording, start, cancel, mute, promote, and demote?
10. Which other host tools are on the bar (kick, lock, invite, retitle)?
11. Is the LiveKit token one per join, and does it expire while the room is open?
12. Should an ended room's recording link replace the live badge, or sit beside it?
