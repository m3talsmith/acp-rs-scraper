# Fixtures: Red Star root reply (P0 capture)

Source: one real reply by @m3talsmith on thread `AaFV7IhwdC-iC6JuW97QMg`
("ACP Red Star Rebranding: Hazaldin Edition"), captured in Chrome on 2026-10-19
19:35–19:40 EDT, redacted with `redact_har.py` and gated with `scan.sh` (CLEAN).
The raw HAR has been shredded. Placeholders look like `<REDACTED:rule#n>`, and the same
value always maps to the same placeholder. Leave those placeholders as they are.
Do not replace them with live session, cookie, or CSRF values.

| file | contents |
|---|---|
| `reply_join.json` | thread LV `phx_join` (url, params incl. `_csrf_token`/`_track_static`/`_mounts`/`_mount_attempts`, session, static, sticky), its full rendered reply, and the sticky `lv:js-spaceBar` join |
| `reply_validate.json` | first and last `form:validate` of 332, each with its `phx_reply` |
| `reply_save.json` | `form:save`, its reply, push_events `thread:insert` + `saving`, the trailing validate → `unmatched topic`, `live_redirect`, leave/close, re-join + reply |

Each frame item has `jsonl_line` (into the redacted capture), `t_iso` (UTC),
`direction`, the raw `frame`, and for form events `form_fields_decoded` + `body_input_parsed`.

No `notifications.html`: /rs/notifications was reached by live navigation (phx_leave +
phx_join with `redirect`), so there is no HTTP HTML body. The rendered notifications tree
contains other members' notification text, so it is not in this directory. The full
`capture.redacted.jsonl` is not committed either.

The reply tests load these three files. They compare the composed validate and save
payloads to the captured frames. They do not open a socket.
