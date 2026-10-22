# Plan: Organizations

Status: **plan, 2026-10-22.** Nothing in this document is implemented yet. The plan is on the organizations draft, ahead of the implementation commits. Gated access (3.48.0) is part of this plan. The implementation commits follow, one minor at a time.

Base: `main` at `274a9e01` (crate `3.39.0`). This plan does not bump the version. `AGENTS.md` bumps a version for a feature or a fix, not for a plan.

An organization is a local content source. It does not need the ACP Red Star service package, and it does not talk to acp.us. Its public face is a website: a template, pages, a feed, and downloads. Another Red Star install can add that website as a service. Private content stays on the machine that owns it.

A public org can also be gated. Public versus private still decides which rows exist for members. Gating decides whether a non-member may see any of those rows. Register and login run in the Red Star app, against the org's service. The browser never holds a private key.

The core stays white-label. The built-in Red Star theme can still supply the default look. A `plain` theme, or any other theme, can replace it. Federation, replication, and the network transport are stubs. They get no version.

## 1. Overview and goals

- A person can create an organization on this machine and post in it with no service installed.
- The org is a source. Its posts show up wherever source content shows up, with the org as the author.
- Visibility is private unless someone deliberately makes it public. The same rule covers the org, each post, and each asset.
- A public org, rendered through its template, is a static site: index, post pages, asset and download pages, and a feed. Any node can export that site to a folder or serve it.
- A public org can publish itself as a service. Another install adds it and browses it like any other source. Posting into it is allowed only when the org says so and the caller's role allows it.
- Members are Red Star users with their own identity keys. Invites are an authenticated Diffie-Hellman exchange. Roles decide what a member can change.
- The person who creates an org is its owner. The org starts with that one owner. The last owner cannot be removed.
- A public org can require membership before any content is shown. Non-members then get a landing page with register and login, and no posts, titles, or asset names. A private org stays closed.
- The key exchange and the login signature run in the Red Star app, talking to the org service. Private keys stay in the app.
- Private material never appears in a public render, a serve response, an export, or the service endpoint.

What the owner sees in the terminal UI and in `--org-show` is the local copy, including private rows. That is not a public render. Every path that writes a page, a file for someone else, or a network response uses the public filter in section 3.

## 2. Organizations

Create, edit, and delete from the command line and from the terminal UI.

- The creator's identity is added as a member with the built-in owner role at the moment the org is created. That lands with members (3.46.0), because that is when an identity key exists. Orgs created by the earlier minor are backfilled: the first time 3.46.0 opens the database, any org with no members gets the local identity as its owner.
- An org has a slug (the source id) and a display name. The slug is the author nick on posts.
- Description is optional.
- Visibility is `private` or `public`. The default is `private`.
- `gated` is off by default. Turning it on is a separate step, in 3.48.0, and it does not by itself make the org public. Section 10.
- Each org has a default for new posts, also `private` until switched.
- Delete removes the org, its posts, its assets, its templates, its invites, and its memberships. It asks first. Cancel is the default.

Making an org public is a live exposure. The command prints what would become readable, then asks. `--dry-run` prints that list and writes nothing. `--yes` confirms. Without a terminal, and without `--yes`, the change is refused.

## 3. Posts

Posts are rows in the local database, not files beside it. The org slug is the source and the author.

- Each post has its own visibility. The default is the org's default, which starts as `private`.
- Switching the org default to `public` does not rewrite posts already stored. It only applies to posts created after the switch. A person can still mark one new post private.
- Readability for anyone who is not looking at the owner's local screens goes through one function, `readable_by_others(org, item, viewer)`. The viewer is anonymous or a member. `--export`, the site folder, `serve`, and the service endpoint call it. They do not grow their own filters.
  - The function is false when the org is private, when the item is private, or when the org is gated and the viewer is not a member.
  - It is true when the org is public, the item is public, and either the org is not gated or the viewer is a member.
  - A public org that is not gated is readable by anyone, except posts and assets that are themselves private.
  - A public org that is gated is not readable by a non-member. That person gets the landing page in section 10 and nothing else: no bodies, no titles, no asset names. A member sees the same public posts and public assets an ungated public org would show. Private rows stay hidden from that member on these paths.
  - A private org is never readable. A post marked public inside a private org stays hidden. Gating does not open it. Membership does not open it on the website, on `serve`, in an export, or on the service.
- The owner's nick bar, search, and `--org-show` include private posts. They are labeled private. Those screens do not call `readable_by_others`. They are the local copy.

## 4. Members and identity

A member is a Red Star user. Each user has a long-term identity, generated on first use:

- Ed25519 for signing (`ed25519-dalek`).
- X25519 for Diffie-Hellman (`x25519-dalek`).
- Both seeds are sealed at rest (section 12). Nothing here hand-rolls a curve or a hash.

Adding a user is an authenticated exchange. Each side's X25519 public value is signed by that side's Ed25519 identity key, so a substituted public key fails verification.

1. The inviter writes an invite: org slug, role, expiry, a single-use id, the inviter's Ed25519 public key, the inviter's X25519 public key, and the signature over those fields.
2. The invitee checks the signature, answers with their own Ed25519 public key, X25519 public key, and signature.
3. Both sides run X25519, then HKDF-SHA256 (`hkdf` 0.12, already in the tree) with a fixed info string and the transcript (invite id and both public keys). The output is not shown.
4. Both sides show the same short fingerprint of that transcript. The person compares the two screens out of band and types the code. If the typed code does not match the computed code, the member is not added.
5. The inviter marks the invite used. A second accept of the same invite is refused. An invite past its expiry is refused.

Local transport, in the mocked tests, is a file the two throwaway `HOME`s share, or a localhost socket bound to `127.0.0.1`. The decentralized transport is a stub with no version.

Invites do not contain private keys. Logs do not contain seeds, shared secrets, or the HKDF output.

Website registration (section 10) is this same exchange. The peer is the organization's service, not another person's invite file, and not the browser. The browser only shows a single-use code. The app opens that code and talks to the service. Both seeds stay inside a Red Star app.

## 5. Roles and permissions

An org can create custom roles and assign them. A permission is one of:

| Permission | What it allows |
| --- | --- |
| `post` | Create a post |
| `edit_own` | Edit a post this member authored |
| `edit_any` | Edit any post in the org |
| `delete_own` | Delete a post this member authored |
| `delete_any` | Delete any post in the org |
| `post_visibility` | Change a post's visibility |
| `org_visibility` | Change the org's visibility, its default post visibility, its name, and its description |
| `assets` | Add, remove, and change the visibility of assets |
| `templates` | Edit templates and the theme values the template uses |
| `publish` | Publish or withdraw the org as a service, and allow or refuse remote posts |
| `invite` | Create an invite, and approve or reject a registration that is waiting |
| `remove_members` | Remove a member who is not the last owner |
| `manage_roles` | Create, edit, and delete custom roles, and assign a role |
| `manage_access` | Set or clear `gated`, choose open registration or approval required, and set the default role a new registration receives |

Built-in roles, shipped with every org:

| Role | Permissions |
| --- | --- |
| owner | All of the above, including `manage_access`, and the only role that may assign `owner` |
| admin | All except assigning `owner`, so an admin has `manage_access`. An admin cannot change an owner's role or remove an owner |
| editor | `post`, `edit_own`, `edit_any`, `delete_own`, `assets`, `templates` |
| member | `post`, `edit_own`, `delete_own` |
| read-only | None of the write permissions. This is the default role for a person who registers through the website |

`manage_access` arrives with gated access (3.48.0). The earlier minors do not have the flag.

The default registration role cannot be `owner`. The person who sets it must hold every permission on the chosen role. That is the same escalation rule as assigning a role. The shipped default is `read-only`.

Custom roles hold a subset of those permissions. The person saving the role can include only permissions they hold. Assigning a role requires `manage_roles` and every permission on that role. Nobody can grant themselves a permission they do not already have.

There is always at least one owner. These are refused:

- Removing the last owner.
- Changing the last owner's role to anything else.
- Deleting the built-in `owner` role.

Deleting the org is a separate confirmed action. It is owner-only. It is not a way to sneak past the last-owner check by removing the member row first.

Every org action checks the actor, including a post that arrives through the service endpoint. A remote caller is the identity that signed the request. If that key is not a member, or the role lacks the permission, the call is refused. The service being published does not widen the role.

The creator-is-owner rule is the same check in reverse: `create` inserts the local identity as `owner` before it returns. A new org cannot exist with zero owners.

## 6. Templates and the theme editor

An org renders HTML through a template. The template editor sits next to a live preview, the same arrangement as the theme editor (Shift-F5): the source on one side, the rendered page on the other, updated as the text changes. Styling in that editor goes through ratatui-lipgloss.

The preview uses the theme that is on screen. The template receives that theme's colours as CSS variables, and the theme's `[brand]` title and motto. Editing the theme still happens in the theme editor. The template preview follows it.

The shipped default template is white-label:

- Headers ask for Cinzel, then a generic serif.
- Body text asks for Lora, then a generic serif.
- The core does not embed those font files and does not paint Cinzel or Lora in the terminal chrome.
- The page title and the motto come from the active brand. The `plain` brand does not gain a Red Star motto by being used as a template.
- An org can replace the default with its own template.

The template is a small substitution language: the org name, description, theme variables, a posts loop (title, body, permalink), and an assets loop (name, download URL). It is not a general programming language.

## 7. Assets

An org can attach media to its assets. Each asset is private by default and can be made public.

- An asset can be placed in a template.
- A public asset has a download page in the site export and a download route on `serve`.
- A private asset is omitted from templates that are rendered for anyone else, from download pages, from the feed, and from the service.
- Making an asset public uses the same preview, confirm, and `--dry-run` path as making a post public.
- A private org hides every asset, even ones marked public.
- A gated public org hides every asset name and every download URL from a non-member. A member still sees only the assets `readable_by_others` allows.

## 8. Website

A public org that is not gated, rendered through its template, is a complete static site:

- `index.html`, the front page.
- One HTML page per public post.
- One download page per public asset, plus the file bytes.
- `feed.json`, and a Markdown file per public post. JSON and HTML are the core's export formats today. Markdown is written as text files beside them. Whether `--format` grows an `md` value is an open question (section 16).
- `--org-export SLUG DIR` writes that tree. A private org writes an empty public site and says so. Private posts and private assets are counted in the summary and left out of the files.
- `--org-serve SLUG` binds `127.0.0.1` by default and serves that same public tree. A private org does not serve content.

A gated public org is not that static site. `serve` is what enforces the gate. Export of a gated org writes only the public landing page (register and login, no posts, no titles, no asset names, no feed entries). A member-only export is a separate confirmed action and is specified in section 10. It is not the default, and it is not a file a stranger can open and read as the public site.

Any later node that can read the public bundle can serve it. That is the "every node can serve" rule. For a gated org the public bundle is the landing page only. The code that would replicate the bundle onto another node is the stub in section 11.

## 9. Organization as a service

Publishing is off by default.

A public org can publish a service manifest and a localhost endpoint. The manifest is the core plugin record: a service id, a display name, the endpoint, and the capabilities the org is willing to offer (`reader`, and `post` only when remote posts are allowed). It is stored as `~/.acp/services/org-<slug>.toml`. The endpoint speaks the same records the plugin crate already has (`Page`, `ItemRec`, `Draft`).

Another install adds it with `--org-add URL`. That install then browses the org like any other source. The read goes through `Reader` and the readiness model (`ready`, `off`, `gated`, and the rest). `Post` is `off` until the org allows remote posts. A remote post is checked against the caller's role (section 5). Only rows `readable_by_others` allows are returned.

When the org is public and gated, an anonymous caller gets readiness `gated` and an empty page. No titles, no asset names, no bodies. The same caller, after the login in section 10, is a member and then receives the public rows their role allows. Readiness `gated` here is the plugin's existing availability value. It is not the changelog marker `prepared, gated:`, and it is not a version. A private org stays unreadable on this path whether or not the flag is set. The service does not widen the role and does not skip the gate.

Publishing, withdrawing, and turning remote posts on are live writes. The preview lists the public posts and public assets that would be exposed, names the capabilities, and asks. Cancel is the default. `--dry-run` prints the manifest and does not write it and does not listen. `--yes` confirms.

The in-process adapter implements the existing `Service`, `Reader`, and `Post` traits. It is not registered as a built-in at startup. This binary still ships with no built-in site service. The adapter is loaded only after a person adds the manifest.

## 10. Gated access

Gated access is its own minor, 3.48.0. It sits on top of the website, the service, members, and roles. The flag does nothing useful until those exist, so it is not folded into an earlier minor.

`gated` is off by default. A role with `manage_access` can set it or clear it. Turning it on, or turning it off, prints what a non-member would start or stop seeing, then asks. Cancel is the default. `--dry-run` prints that and writes nothing. `--yes` confirms. Without a terminal, and without `--yes`, the change is refused.

Two switches, and they do not collapse into one:

- Public versus private decides which rows a member may see. The rules are the function in section 3.
- Gating decides whether a non-member gets anything at all.

A private org stays never-readable. Setting `gated` on a private org does not publish a landing page and does not reveal that the org has posts. The public paths still say nothing.

A public org with `gated` off is the site in section 8.

A public org with `gated` on requires membership for every content path: reading posts, the website beyond the landing page, assets and downloads, the feed, `serve`, an export pull, and the org-as-service path. A non-member receives one minimal landing page. That page has the org's display name, a register action, and a login action. It has no post bodies, no post titles, and no asset names. The name is on the page so the person can see which org they are about to join. Hiding the name as well is an open question (section 16).

### Register

The browser does not run Diffie-Hellman, does not call WebCrypto, and does not import a key file. Private keys live only in the Red Star app.

1. The website, on `serve`, creates a registration request. It is single-use and short-lived (10 minutes, unless section 16 changes that). The page shows it as a pasteable code and as a `redstar://org-register` link with the same code and the service origin. A QR image of that link is fine. The code is an id, not a key.
2. The request is bound to the browser session that created it. That session is an HttpOnly, SameSite=Strict cookie, Secure when the page was served over HTTPS. A code presented from any other browser session is refused. Completing it does not log that other session in.
3. The person opens the code in their Red Star app (the link, or paste). The app shows the org name and the service origin and waits for a yes before it talks. Cancel is the default.
4. The app then runs the authenticated exchange from section 4 directly with the org-as-service endpoint. Each side's X25519 public value is signed by that side's Ed25519 identity. HKDF-SHA256 derives the transcript key. Both sides show the fingerprint. The app will not finish until the person types a fingerprint that matches. The service shows the same fingerprint on the org's local screens.
5. The service creates the member. The org chooses open registration or approval required. The default is approval required, so the new row is `pending` and has no access yet. Approval goes to a role with `invite`, and the approver sees the fingerprint before they accept. Open registration activates the member as soon as the fingerprint matches. The role on that row is the org's default registration role, which starts as `read-only`.
6. The waiting browser page polls `serve` and updates when the exchange completes: registered, or pending approval. The poll is accepted only with the cookie from step 2. The page never receives a private key, a seed, a shared secret, or the HKDF output.

The service side of the exchange uses a dedicated org service identity (Ed25519 and X25519), generated the first time the org is published, sealed like a user identity (section 12). The public key is in the manifest. The app shows that key's fingerprint as the org it is joining. Using the owner's personal key for this is an open question (section 16).

If the org is not published, there is no service endpoint to exchange with. The landing page says registration is unavailable. Local invites (section 4) still work on the owning machine.

The endpoints that create a code, poll it, and submit a signature are rate-limited per org and per browser session. They require a same-origin CSRF token bound to that browser session. SameSite=Strict is extra, not the only check.

### Login

Login is not a second Diffie-Hellman exchange, and it is not a password.

The website shows a single-use login request, bound to that browser session the same way a registration code is, with the same lifetime, the same CSRF check, and the same rate limit. The person opens it in the Red Star app. The app shows the org name and the origin, then signs the service's nonce with the user's Ed25519 identity key and sends the signature to the org service. The service checks the signature against the stored public key, checks that the code is unused, unexpired, and bound to the browser session that is polling, and approves that session.

`serve` then sets a short-lived session cookie: HttpOnly, SameSite=Strict, Secure when the page is HTTPS. The plan uses 8 hours (section 16). Logout clears it. After expiry the cookie is rejected and the person is anonymous again. No secret is stored in the browser beyond that cookie. The cookie's value is an id. The server stores the session.

From then on, every path uses the member's role (section 5). A read-only member can read public rows and cannot post.

The terminal UI and the command line log in to an org service with the same challenge-response and no browser step: the app asks the service for a nonce, signs it, and keeps the resulting session for that process. The org-as-service path accepts that same proof. A caller with no valid session is anonymous.

A replayed code, an expired code, a code whose browser session does not match, and a signature from the wrong key all fail. They create no member and no session.

### Export and serve

`serve` is the component that gates. A static folder cannot check membership, so a gated org cannot be exported as a plain copy of member content.

- `--org-export` on a gated public org writes the landing page only.
- `--org-export SLUG DIR --org-member-export` writes the rows a member would see (public posts and public assets, still no private rows). It asks first. The preview says the folder is member content and must not be published as the open site. Cancel is the default. `--dry-run` lists the files and writes nothing. `--yes` confirms.
- Asset, download, and feed routes on `serve` return the landing response, or an empty gated response with no names, when the caller is anonymous. They do not redirect to a URL that contains a title.
- A private org still serves and exports nothing.

## 11. Network

Every node that has the public bundle can serve it in the formats section 8 lists. That serving is local in this stack (`127.0.0.1` unless a later, explicit setting says otherwise).

These are stubs. They are not features. They get no version. The first release that has a place to mention them records them under Added as `prepared, gated: …`:

- Federation and replication of org content between nodes.
- The decentralized transport for invites and for the service endpoint. Files and localhost are the transports that actually run.

No packet is sent to acp.us. Tests use a throwaway `HOME` and, where two installs are required, two throwaway homes.

## 12. Security and privacy

- `readable_by_others` is the only public filter. Render, serve, export, and the service call it. Tests cover every combination: private org, public org with a private post, public org whose default is public, private assets, and a gated public org seen by a non-member and by a member.
- The browser never holds a private key. Registration and login codes are single-use, expiring, and bound to the browser session that created them. A different session cannot finish them.
- Session cookies are HttpOnly and SameSite=Strict, and Secure when the page is HTTPS. Logout and expiry both end the session.
- Registration and login endpoints are rate-limited and require a same-origin CSRF token.
- Identity seeds are sealed with XChaCha20-Poly1305 under a key derived from the unlocked database key, the same construction as the node keystore (`keystore_key` in `acp_red_star_node_core`). The file mode is `0600`. The directory mode is `0700`.
- Seeds, shared secrets, and the HKDF output never appear in logs, exports, HTML, serve bodies, service JSON, or test snapshots. `Debug` for the key type prints `[redacted]`.
- An invite file holds public keys and signatures only.
- Confirmations that expose data (public visibility, publish, remote post, gating, a member-only export) default to Cancel.
- Deletes default to Cancel.
- A fingerprint mismatch, an expired invite, and a replayed invite add nobody.

## 13. Data model, commands, and screens

Tables, created on first use in the encrypted database. Names can still move if a migration is cleaner. The columns are the contract.

```text
organizations
  id, slug unique, name, description,
  visibility,            -- private | public, default private
  gated,                 -- 0 by default; 3.48.0
  registration,          -- approval | open, default approval; 3.48.0
  default_member_role,   -- built-in read-only until changed; 3.48.0
  default_post_visibility,
  allow_remote_post,     -- 0 by default
  published,             -- 0 by default
  created_at, updated_at

org_posts
  id, org_id, author_identity,
  title, body,
  visibility,            -- default copies the org default at insert time
  created_at, updated_at

org_assets
  id, org_id, name, mime, visibility, path, created_at

org_templates
  id, org_id, name, html, updated_at

org_roles
  id, org_id, name, permissions, builtin

org_members
  org_id, identity_pub, role_id,
  status,                -- active | pending; invites are active, open registration is active, approval starts pending
  added_at

org_invites
  id, org_id, role_id, inviter_pub,
  expires_at, used_at, nonce

org_access_requests
  id, org_id, kind,      -- register | login
  browser_session, code, nonce,
  expires_at, used_at,
  identity_pub,          -- set when the app finishes the exchange or the signature
  csrf_token

org_sessions
  id, org_id, identity_pub, expires_at
```

`org_access_requests` and `org_sessions` arrive in 3.48.0. The service identity file is not a table. It is sealed next to the user identity, mode `0600`, and it is created when the org is first published.

The identity file is not a table. It lives under the home directory, sealed, mode `0600`.

Public identity keys are hex. Slugs are the nick stored on a `Post` when the terminal UI merges org posts into the nick bar. Post ids are prefixed so they do not collide with scraped ids.

Command line, grouped by the minor that adds them. Flags that only preview keep `--dry-run`. Flags that expose or delete accept `--yes` and otherwise ask.

Orgs and posts (3.40.0):

- `--org-list`
- `--org-create SLUG` with `--org-name` and `--org-description`
- `--org-edit SLUG`
- `--org-delete SLUG`
- `--org-show SLUG` (owner view, private rows included and labeled)
- `--org-post SLUG` with `--org-title` and `--body`
- `--org-visibility private|public`
- `--org-default-visibility private|public`
- `--org-post-visibility private|public` on a new post

Privacy (3.41.0) uses those same flags and adds the public export check that later minors reuse. The four visibility cases are tested here, against a public JSON snapshot, before HTML exists.

Templates (3.42.0):

- `--org-template-show SLUG`
- `--org-template-set SLUG --file PATH`
- `--org-template-reset SLUG` restores the default

Assets (3.43.0):

- `--org-asset-add SLUG --file PATH`
- `--org-asset-list SLUG`
- `--org-asset-visibility ID private|public`
- `--org-asset-remove ID`

Website (3.44.0):

- `--org-export SLUG DIR`
- `--org-serve SLUG` with `--org-serve-addr` (default `127.0.0.1` and an ephemeral or documented port)

Service (3.45.0):

- `--org-publish SLUG`
- `--org-unpublish SLUG`
- `--org-allow-posts SLUG` and `--org-refuse-posts SLUG`
- `--org-add URL`
- `--org-services`
- `--org-browse SLUG`

Members (3.46.0):

- `--org-identity` prints the public fingerprint and nothing else
- `--org-invite SLUG --role NAME --org-invite-out PATH`
- `--org-invite-accept PATH --org-fingerprint CODE --org-invite-reply PATH`
- `--org-invite-finish PATH --org-fingerprint CODE`
- `--org-members SLUG`
- `--org-invite-serve` for the localhost exchange

Roles (3.47.0):

- `--org-roles SLUG`
- `--org-role-create SLUG NAME --permissions LIST`
- `--org-role-assign SLUG MEMBER ROLE`
- `--org-member-remove SLUG MEMBER`

Gated access (3.48.0):

- `--org-gate SLUG` and `--org-ungate SLUG`
- `--org-registration SLUG approval|open`
- `--org-default-role SLUG ROLE`
- `--org-registrations SLUG` lists pending rows
- `--org-registration-approve SLUG MEMBER` and `--org-registration-reject SLUG MEMBER`
- `--org-login URL` signs the service nonce from this install, with no browser
- `--org-open-code CODE` is what the app runs for a register or login code. It prints the org name, the origin, and the fingerprint, and it asks before sending
- `--org-member-export` on `--org-export`, the confirmed member folder

Terminal UI. All of it is styled with ratatui-lipgloss. Each screen has a tooltip and a Help topic.

- Library › Organizations. List, create, edit, delete. The create form shows that the creator becomes the owner.
- The nick bar gains the org as a source. Opening it lists that org's posts. Private rows are marked.
- Organizations › Template. Source and live preview, side by side. The preview uses the theme on screen.
- Organizations › Assets.
- Organizations › Members. Roster, invite, accept, and the fingerprint prompt.
- Organizations › Roles. Built-in roles, custom roles, assignments. The last owner is marked and cannot be cleared from this screen.
- Organizations › Access. The gated flag, open versus approval, the default registration role, and the pending list. Approving a pending member shows the fingerprint. Opening a code shows the org name and origin before the yes.
- A confirm modal, Cancel focused, for making something public, publishing, deleting, removing a member, gating or ungating, and a member-only export.

Help topics: `orgs`, `org-posts`, `org-templates`, `org-assets`, `org-serve`, `org-service`, `org-members`, `org-roles`, `org-access`.

## 14. Test plan

Mocked Gherkin only. Each scenario uses a throwaway `HOME`. None of them contact acp.us. Two-party scenarios use two throwaway homes. A localhost serve stays on `127.0.0.1`.

Organizations and posts:

- Create an org and see it in `--org-list` and in the nick list.
- Edit the name and description.
- Post into the org and read it back from `--org-show`, with the org as the author.
- Delete asks, and `--dry-run` deletes nothing.
- A new org and a new post are private unless a flag says otherwise.

Privacy:

- A private org contributes nothing to a public snapshot.
- A public org with a private post omits that post and includes a public one.
- A public org whose default is public includes a new post that did not set its own visibility.
- A public post inside a private org is still omitted.
- Switching the default does not rewrite an older private post.

Templates:

- The default template names Cinzel and Lora and takes its title from the brand, not from a hard-coded product string.
- The `plain` brand's render does not contain the Red Star motto.
- A template edit is visible in the live preview text.

Assets:

- A new asset is private.
- A private asset is absent from the public render and from the download list.
- A public asset in a public org is present and downloadable.
- A public asset in a private org is absent.

Website and serve:

- `--org-export` writes `index.html`, a post page, a download page, and `feed.json`, and omits private rows.
- `--org-serve` on `127.0.0.1` returns those public pages and does not return a private post or a private asset.
- A private org serves nothing.

Service:

- Publish is off until asked. `--dry-run` prints the manifest and does not write it.
- After a confirmed publish, a second throwaway `HOME` adds the localhost endpoint and browses only the public posts.
- A remote post is refused when remote posts are off.
- A remote post is refused when the caller's role lacks `post`.
- A remote post from a member with `post` is stored, and a private result is not served back.

Members and invites:

- Creating an org adds the creator as a member with the owner role, and the org has one owner.
- Invite, accept, and finish with a matching fingerprint adds the member.
- Reject, or a fingerprint that does not match, adds nobody.
- An expired invite is refused.
- Finishing the same invite twice is refused.
- The identity file is mode `0600`.
- A public export, a serve body, and a service page do not contain the identity seed.

Roles:

- The five built-in roles exist on a new org.
- A member cannot grant a permission they do not hold.
- A member cannot assign themselves `owner`.
- Removing the last owner is refused.
- Changing the last owner's role is refused.
- An editor can edit any post and cannot publish the org.
- A read-only member cannot post.
- The service path enforces the same matrix.

Gated access. Two throwaway homes. One home owns the org and runs `serve` plus the org service on `127.0.0.1`. The other home is the Red Star app. The browser is the test client calling `serve`. Nothing contacts acp.us.

- A non-member is blocked on the site, on an asset URL, on a download, on the feed, on `serve`, and on the service. The response has no post titles and no asset names.
- Toggling `gated` off makes the public rows readable again. Toggling it on hides them. `--dry-run` changes nothing. A non-terminal refuses the toggle without `--yes`.
- A private org stays empty on those paths when `gated` is on and when it is off.
- Open registration: the app and the service complete the exchange, the fingerprint matches, and the new member is active with the read-only role.
- Approval required, the default: the same exchange leaves the member pending, and a role without `invite` cannot approve them. After approval the member can read.
- A replayed registration code is refused. An expired code is refused. A code polled or finished from a different browser session is refused.
- Login: the app signs the nonce, the browser session that created the code receives the cookie, and a later read succeeds.
- A replayed or expired login nonce is refused. A signature from the wrong key is refused. No session is issued.
- After the session expires, the same cookie is anonymous again.
- A read-only session can read a public post and cannot post.
- The landing page and the service JSON from these scenarios do not contain a seed, a private key, or the HKDF output.
- `--org-export` of a gated org contains the landing page only. `--org-member-export --dry-run` writes nothing.

Unit tests sit beside the modules for the readability function, the signature check, the HKDF transcript, and the permission lattice. Gherkin covers the paths above. `cargo test`, `cargo clippy -- -D warnings`, and the mocked end-to-end suite pass on each minor. MSRV stays 1.88.

## 15. Delivery

Stacked draft pull requests, each based on the one above it, the first based on `main`. One minor each. The plan commits are the first commits on the first of these and do not bump the version.

| Version | Pull request | What a person can do |
| --- | --- | --- |
| none | this plan | Read the plan |
| 3.40.0 | orgs and posts | Create, edit, and delete orgs and posts. See the org as a source. Defaults are private |
| 3.41.0 | privacy | Public snapshot refuses every private combination. The per-org default can be switched |
| 3.42.0 | templates | Edit a template with a live preview. The default asks for Cinzel and Lora and stays white-label |
| 3.43.0 | assets | Add media, keep it private, and use public assets in the template |
| 3.44.0 | website | Export a static site and serve it on localhost. Public rows only |
| 3.45.0 | service | Publish a manifest and endpoint. Another install adds it. Remote posts follow the role |
| 3.46.0 | members | Identity keys, invites, fingerprints. The creator is the owner |
| 3.47.0 | roles | Built-in and custom roles. Last-owner protection. No escalation |
| 3.48.0 | gated access | Membership required on a public org. Register and login in the app, against the service. The browser holds a session cookie and no key |

Stubs, with no version of their own, recorded as `prepared, gated:` on the release that first has somewhere to put the sentence (3.44.0 for replication, 3.46.0 for invite transport):

- Federation and replication of org content.
- Any transport that is not a file or `127.0.0.1`.

Plugin crate version stays where `main` left it. These minors are the root crate.

## 16. Open questions

1. Invite lifetime. The plan uses 48 hours unless you want a different default.
2. Identity key. The plan generates a new Ed25519/X25519 pair for org membership, separate from the node keystore, so rotating the node does not rewrite memberships. Say if membership should instead be the node key.
3. Diffie-Hellman keys. The plan uses the long-term X25519 key, signed by the identity key. An ephemeral X25519 per invite is stricter and means one more secret to throw away after HKDF. Say if you want ephemeral.
4. Asset bytes. The plan stores files under the home directory at mode `0600` and refuses to copy private files into an export. Sealing the bytes the way the identity seed is sealed is safer and heavier. Say which you want.
5. Markdown. The site writes `.md` files next to HTML and JSON. Say if `--format md` should become a third export format in the core.
6. Whole-database `--export`. The plan leaves org posts out of a nick dump unless the export is `--org-export` or the selected source is that org, and even then only public rows are written. Say if a full dump should include public org posts automatically.
7. Default serve port. The plan binds `127.0.0.1` and will pick a fixed port in the serve minor so tests can find it. Say if you already have a port reserved.
8. Remote posts. A member with `post` who posts through the service creates a post at the org's default visibility. A private result is stored and is not served. Say if remote posts should be forced public, since they arrived on the public endpoint.
9. Admin and owners. An admin cannot change an owner. Say if two owners should be able to demote each other as long as one owner remains.
10. Fonts. Cinzel and Lora are named in CSS and not vendored. Say if the default template should ship the font files.
11. Registration and login codes last 10 minutes. The session cookie lasts 8 hours. Say if you want different lifetimes.
12. The landing page shows the org's display name plus register and login, and nothing else. Say if the name should be hidden too.
13. The service signs the registration exchange with a dedicated org key, created when the org is first published. Say if that signature should be the owner's personal identity key instead.
14. Rate limit. The plan refuses a browser session after 10 registration attempts and 10 login attempts in a minute, and the org after 30 of each in a minute. Say if you want different numbers.
