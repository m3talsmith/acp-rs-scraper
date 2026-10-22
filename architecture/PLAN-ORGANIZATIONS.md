# Plan: Organizations

Status: **3.43.0 ships files.** 3.42.0 ships templates. 3.41.0 ships the public snapshot. 3.40.1 confirms guest writes, 2026-10-22. Michael's final defaults are in this document and replace the earlier ones. A new org is gated and private. Registration is Closed. The built-in roles are Owner, User, and Viewer, and the creator is the Owner. A public org that is not gated accepts a write from any client. That rule is confirmed below and is no longer an assumption. The website, the service, invites, and the browser flow are still later minors. Plan commits do not bump the version. Implementation commits follow, one minor at a time.

Base: `main` at `274a9e01` (crate `3.39.0`). This plan does not bump the version. `AGENTS.md` bumps a version for a feature or a fix, not for a plan.

An organization is a local content source. It does not need the ACP Red Star service package, and it does not talk to acp.us. Its public face is a website: a template, pages, a feed, and downloads. Another Red Star install can add that website as a service. Private content stays on the machine that owns it.

A new org is gated and private. Public versus private still decides which rows members can see. Gating decides whether a non-member gets anything at all. Register and login run in the Red Star app, against the org's service. The browser never holds a private key.

The core stays white-label. The built-in Red Star theme can still supply the default look. A `plain` theme, or any other theme, can replace it. Federation, replication, and the network transport are stubs. They get no version.

## 1. Overview and goals

- A person can create an organization on this machine and post in it with no service installed.
- The org is a source. Its posts show up wherever source content shows up, with the org as the author.
- A new org is private and gated. Both stay that way until someone changes them. The same private default covers each post and each asset.
- A public org, rendered through its template, is a static site: index, post pages, asset and download pages, and a feed. Any node can export that site to a folder or serve it.
- A public org can publish itself as a service. Another install adds it and browses it like any other source. Posting into a gated org is allowed only when the caller's role allows it. A public org that is not gated also accepts a write from a client that is not a member. Section 9.
- Members are Red Star users with their own identity keys. Invites are an authenticated Diffie-Hellman exchange. Roles decide what a member can change.
- The person who creates an org is its Owner. The org starts with that one Owner, plus the built-in User and Viewer roles. The last Owner cannot be removed.
- A gated org requires membership before content is shown. Non-members then get a landing page with register and login, and no posts, titles, or asset names. A private org stays closed. A public org that is not gated is the exception in section 9.
- The key exchange and the login signature run in the Red Star app, talking to the org service. Private keys stay in the app.
- Private material never appears in a public render, a serve response, an export, or the service endpoint.

What the owner sees in the terminal UI and in `--org-show` is the local copy, including private rows. That is not a public render. Every path that writes a page, a file for someone else, or a network response uses the public filter in section 3.

## 2. Organizations

Create, edit, and delete from the command line and from the terminal UI.

- Creating an org generates the local identity if this install does not have one yet, then adds that identity as a member with the built-in Owner role. Owner, User, and Viewer are inserted in the same write. The org cannot be saved without them. Invite transport still arrives with members (3.46.0). The owner row does not wait for that.
- An org has a slug (the source id) and a display name. The slug is the source nick. Each post stores an author. A member, or a client that signs with a Red Star identity, stores that public key. A write with no identity stores `anonymous`.
- Description is optional.
- Visibility is `private` or `public`. The default is `private`.
- `gated` is on by default. Turning it off is a live exposure. It does not by itself make the org public. Section 10.
- Registration is `closed` or `open`. Closed is the default. Closed means a new registration waits for approval. Open means it becomes a member immediately. Section 10.
- Each org has a default for new posts, also `private` until switched.
- Delete removes the org, its posts, its assets, its templates, its invites, and its memberships. It asks first. Cancel is the default.

Making an org public is a live exposure. The command prints what would become readable, then asks. `--dry-run` prints that list and writes nothing. `--yes` confirms. Without a terminal, and without `--yes`, the change is refused.

## 3. Posts

Posts are rows in the local database, not files beside it. The org slug is the source and the author.

- Each post has its own visibility. The default is the org's default, which starts as `private`.
- Switching the org default to `public` does not rewrite posts already stored. It only applies to posts created after the switch. A person can still mark one new post private.
- Readability for anyone who is not looking at the owner's local screens goes through one function, `readable_by_others(org, item, viewer)`. The viewer is anonymous or a member. `--export`, the site folder, `serve`, and the service endpoint call it. They do not grow their own filters.
  - The function is false when the org is private, when the item is private, or when the org is gated and the viewer is not a member.
  - It is true when the org is public, the item is public, the item is not archived, and either the org is not gated or the viewer is a member.
  - An archived post or asset is never readable on these paths, whatever its old visibility was.
  - A public org that is not gated is readable by anyone, except posts and assets that are themselves private or archived. What a non-member may write there is the confirmed rule in section 9 (2026-10-22).
  - A public org that is gated is not readable by a non-member. That person gets the landing page in section 10 and nothing else: no bodies, no titles, no asset names. A member sees the same public posts and public assets an ungated public org would show. Private rows stay hidden from that member on these paths.
  - A private org is never readable. A post marked public inside a private org stays hidden. The gated flag does not open it. Membership does not open it on the website, on `serve`, in an export, or on the service. A new org is both private and gated, so both switches are closed.
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

There is no admin, editor, member, or read-only role. Every org is created with three built-in roles, and only those three:

| Role | Place | What it can do |
| --- | --- | --- |
| Owner | Always the top | Every org feature, including the ones added later. The creator is an Owner. |
| User | Between Viewer and Owner | Post, reply, and add attachments to posts and replies. |
| Viewer | Always the bottom | View posts and replies. No writes. |

The permission set is a list of names stored as text, not a closed enum in the database. User starts as `post`, `reply`, `attach`. Viewer starts as `view`. Owner is the set of every permission this build knows, so a later permission arrives with Owner and does not need a new role. A permission this build does not know yet is kept on a custom role and does nothing until the code learns the name.

Other permissions the build already names: `edit_own`, `edit_any`, `archive_own`, `archive_any`, `post_visibility`, `org_visibility`, `assets`, `templates`, `publish`, `invite`, `remove_members`, `manage_roles`, `manage_access`. `invite` is the approve/invite permission: create an invite, and approve or reject a Closed registration. Owner always has it. `archive_any` and `remove_members` are Owner-only in the built-in set. A custom role can be given them only by an Owner.

Custom roles sit strictly between Viewer and Owner. They cannot be placed at Viewer's rank or at Owner's rank, and they cannot be named Owner, User, or Viewer. The person who saves a custom role can include only permissions they hold.

Nobody can grant a role at or above their own rank unless they are an Owner. A User cannot assign User or Owner. A custom role cannot assign itself or anything above it. An Owner can assign Owner. These are still refused:

- Removing the last Owner.
- Changing the last Owner's role to anything else.
- Deleting the built-in Owner, User, or Viewer role.
- Choosing Owner as the role a new member receives.

A new member, from registration or from an invitation, gets the lowest role, which is Viewer, until the org changes that. The org has two settings, and they are not the same field: the role handed out by registration, and the role handed out by an invitation. Either one can be any role except Owner. Changing one does not change the other.

Deleting the org is a separate confirmed action. It is Owner-only. It is not a way to sneak past the last-Owner check by removing the member row first.

Every org action checks the actor, including a post that arrives through the service endpoint. A remote caller is the identity that signed the request. The service being published does not widen the role.

### Owner removal is an archive

When an Owner removes another user's post or asset, the row stays in the database and on the network. It is not deleted.

- It becomes archived and private.
- Custody moves to the organization Owner. The original author stays on the row.
- The row records who archived it and when.
- A replication message for that change is an update, not a delete. The transport is the stub in section 11. It gets no version.
- After that, the row is absent from every public render, serve response, export, feed, and service response. The owner's local screens can still show it, labeled archived.

Removing a member does not delete their posts. Those posts stay attributed to the original author until an Owner archives them.

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

**Confirmed 2026-10-22.** The Red Star app is required for membership only: register, login, the key exchange, and anything a member does as a member. That includes role-based actions, a gated org, and Owner powers. A public org that is not gated can be used by any client — a Red Star app, a browser, curl, or other software — to view, post, reply, and attach. No Red Star identity is required for those four. A write that presents no identity is stored as `anonymous`, and it has no verifiable author. A client that does present a Red Star identity, with a valid signature over the write, is stored under that public key, so the author is verified. Neither kind of non-member can do an Owner-only action: remove a user, or archive a post or asset that belongs to someone else. An Owner archive still applies to an anonymous post or asset, the same as it applies to anyone else's. Guest writes, the ones with no identity, are limited to 10 per source address per organization per minute. A guest body is at most 32 KiB. A guest attachment is at most 8 MiB. A gated org rejects both an unauthenticated write and an identity-signed write from someone who is not a member. A private org rejects them too.

Publishing, withdrawing, and turning remote posts on are live writes. The preview lists the public posts and public assets that would be exposed, names the capabilities, and asks. Cancel is the default. `--dry-run` prints the manifest and does not write it and does not listen. `--yes` confirms.

The in-process adapter implements the existing `Service`, `Reader`, and `Post` traits. It is not registered as a built-in at startup. This binary still ships with no built-in site service. The adapter is loaded only after a person adds the manifest.

## 10. Gated access

Gated access is its own minor for the website and the service cookie, 3.48.0. The flag itself is on from the moment the org is created (3.40.0). A role with `manage_access` can clear it or set it again. Turning it off, or turning it back on, prints what a non-member would start or stop seeing, then asks. Cancel is the default. `--dry-run` prints that and writes nothing. `--yes` confirms. Without a terminal, and without `--yes`, the change is refused.

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
5. The service creates the member. The org's registration mode is Open or Closed. Closed is the default, so the new row is `pending` and has no access yet. Approval goes to a role with `invite`. Owner always has that permission. The approver sees the fingerprint before they accept. Open registration activates the member as soon as the fingerprint matches. The role on that row is the org's registration default, which starts as Viewer. An invitation uses the invitation default, which also starts as Viewer and can be a different role. Neither default can be Owner.
6. The waiting browser page polls `serve` and updates when the exchange completes: registered, or pending approval. The poll is accepted only with the cookie from step 2. The page never receives a private key, a seed, a shared secret, or the HKDF output.

The service side of the exchange uses a dedicated org service identity (Ed25519 and X25519), generated the first time the org is published, sealed like a user identity (section 12). The public key is in the manifest. The app shows that key's fingerprint as the org it is joining. Using the owner's personal key for this is an open question (section 16).

If the org is not published, there is no service endpoint to exchange with. The landing page says registration is unavailable. Local invites (section 4) still work on the owning machine.

The endpoints that create a code, poll it, and submit a signature are rate-limited per org and per browser session. They require a same-origin CSRF token bound to that browser session. SameSite=Strict is extra, not the only check.

### Login

Login is not a second Diffie-Hellman exchange, and it is not a password.

The website shows a single-use login request, bound to that browser session the same way a registration code is, with the same lifetime, the same CSRF check, and the same rate limit. The person opens it in the Red Star app. The app shows the org name and the origin, then signs the service's nonce with the user's Ed25519 identity key and sends the signature to the org service. The service checks the signature against the stored public key, checks that the code is unused, unexpired, and bound to the browser session that is polling, and approves that session.

`serve` then sets a short-lived session cookie: HttpOnly, SameSite=Strict, Secure when the page is HTTPS. The plan uses 8 hours (section 16). Logout clears it. After expiry the cookie is rejected and the person is anonymous again. No secret is stored in the browser beyond that cookie. The cookie's value is an id. The server stores the session.

From then on, every path uses the member's role (section 5). A Viewer can read public rows and cannot post.

The terminal UI and the command line log in to an org service with the same challenge-response and no browser step: the app asks the service for a nonce, signs it, and keeps the resulting session for that process. The org-as-service path accepts that proof for membership actions: a role, a gated org, and Owner powers. A public org that is not gated does not require it to view, post, reply, or attach. Section 9. A caller with no membership session is not a member.

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

- Federation and replication of org content between nodes. An Owner archive is replicated as an update of the archived, private, and custody columns, not as a delete.
- The decentralized transport for invites and for the service endpoint. Files and localhost are the transports that actually run.

No packet is sent to acp.us. Tests use a throwaway `HOME` and, where two installs are required, two throwaway homes.

## 12. Security and privacy

- `readable_by_others` is the only public filter. Render, serve, export, and the service call it. Archived rows fail it. Tests cover every combination: a new org that is gated and private, a private org, a public org with a private post, a public org whose default is public, private assets, a gated public org seen by a non-member and by a member, and an archived row after an Owner removal.
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
  gated,                 -- 1 by default
  registration,          -- closed | open, default closed
  default_register_role, -- Viewer until changed; never Owner
  default_invite_role,   -- Viewer until changed; never Owner; independent of register
  default_post_visibility,
  allow_remote_post,     -- 0 by default
  published,             -- 0 by default
  created_at, updated_at

org_posts
  id, org_id, author_identity, parent_id,
  title, body,
  visibility,            -- default copies the org default at insert time
  custodian_identity,    -- author until an Owner archives it, then the org Owner
  archived,              -- 0 by default
  archived_by, archived_at,
  created_at, updated_at

org_assets
  id, org_id, author_identity, name, mime, visibility, path,
  custodian_identity, archived, archived_by, archived_at,
  created_at

org_templates
  id, org_id, name, html, updated_at

org_roles
  id, org_id, name, permissions, builtin,
  rank                   -- Viewer 0, User 500, Owner 1000; custom ranks are 1..999

org_members
  org_id, identity_pub, role_id,
  status,                -- active | pending; Open registration is active, Closed starts pending
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
- `--org-roles SLUG` (Owner, User, and Viewer exist as soon as the org does)
- `--org-members SLUG` (the creator is the Owner)

Privacy (3.41.0) adds `--org-snapshot SLUG`. It prints the public JSON copy. A private organization contributes no name and no posts. A public organization that is still gated contributes its name and nothing else. Archived rows are left out. Switching the default for new posts does not rewrite posts already stored. Later export, serve, and service paths reuse this snapshot. The four visibility cases are tested here, before HTML exists.

Templates (3.42.0):

- `--org-template-show SLUG` prints the template. The default asks for Cinzel and Lora. The title comes from the brand.
- `--org-template-set SLUG --org-template-file PATH` replaces it. Asks first. Cancel is the default.
- `--org-template-reset SLUG` restores the default. Asks first.

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

- `--org-role-create SLUG NAME --rank N --permissions LIST`
- `--org-role-assign SLUG MEMBER ROLE`
- `--org-member-remove SLUG MEMBER`
- `--org-default-register-role SLUG ROLE`
- `--org-default-invite-role SLUG ROLE`
- `--org-archive POST` archives another user's post. It does not delete it.

Gated access (3.48.0):

- `--org-gate SLUG` and `--org-ungate SLUG` (a new org is already gated)
- `--org-registration SLUG closed|open`
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
- Organizations › Roles. Owner, User, and Viewer, then custom roles between Viewer and Owner. The last Owner is marked and cannot be cleared from this screen. Register default and invite default are two fields.
- Organizations › Access. The gated flag (on for a new org), Open versus Closed, and the pending list. Approving a pending member shows the fingerprint. Opening a code shows the org name and origin before the yes.
- A confirm modal, Cancel focused, for making something public, publishing, deleting, removing a member, gating or ungating, and a member-only export.

Help topics: `orgs`, `org-posts`, `org-templates`, `org-assets`, `org-serve`, `org-service`, `org-members`, `org-roles`, `org-access`.

## 14. Test plan

Mocked Gherkin only. Each scenario uses a throwaway `HOME`. None of them contact acp.us. Two-party scenarios use two throwaway homes. A localhost serve stays on `127.0.0.1`.

Organizations and posts:

- Create an org and see it in `--org-list` and in the nick list.
- Edit the name and description.
- Post into the org and read it back from `--org-show`, with the org as the author.
- Delete asks, and `--dry-run` deletes nothing.
- A new org is private and gated unless a flag says otherwise. A new post is private.
- `--org-roles` on that org lists Owner, User, and Viewer.
- `--org-members` lists the creator as Owner.

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

- A new org has Owner, User, and Viewer, and no other built-in role.
- The creator is the only Owner.
- A new member gets Viewer until the org changes the default. The registration default and the invitation default can be set to different roles. Owner is refused for both.
- A custom role must sit strictly between Viewer and Owner.
- A User cannot assign User or Owner. An Owner can assign Owner. The last Owner cannot be removed or given another role.
- A Viewer cannot post. A User can post, reply, and attach, and cannot archive another user's post or remove a member.

Gated access. Two throwaway homes. One home owns the org and runs `serve` plus the org service on `127.0.0.1`. The other home is the Red Star app. The browser is the test client calling `serve`. Nothing contacts acp.us.

- A non-member is blocked on the site, on an asset URL, on a download, on the feed, on `serve`, and on the service. The response has no post titles and no asset names.
- Toggling `gated` off makes the public rows readable again. Toggling it on hides them. `--dry-run` changes nothing. A non-terminal refuses the toggle without `--yes`.
- A private org stays empty on those paths when `gated` is on and when it is off.
- Open registration: the app and the service complete the exchange, the fingerprint matches, and the new member is active with the registration default (Viewer until changed).
- Closed registration, the default: the same exchange leaves the member pending. A role without `invite` cannot approve them. After approval the member can read.
- A public org that is not gated accepts an unauthenticated guest post. The stored author is `anonymous`. The same org accepts a non-member post signed with a Red Star identity key, and that stored author is the key. Neither caller can remove a member or archive someone else's post. An Owner can archive the anonymous post. A gated org, including one that is public, rejects both the guest write and the signed non-member write. The eleventh guest write from one source address to one org inside a minute is refused. A different source address, and the same address on a different org, are not. A body over 32 KiB and an attachment over 8 MiB are refused.
- An Owner archive leaves the row in the database, marks it archived and private, sets custody to the Owner, and keeps the original author. A public snapshot, a serve body, an export, a feed, and a service response do not contain it. The replication record is an update, not a delete.
- A replayed registration code is refused. An expired code is refused. A code polled or finished from a different browser session is refused.
- Login: the app signs the nonce, the browser session that created the code receives the cookie, and a later read succeeds.
- A replayed or expired login nonce is refused. A signature from the wrong key is refused. No session is issued.
- After the session expires, the same cookie is anonymous again.
- A Viewer session can read a public post and cannot post.
- The landing page and the service JSON from these scenarios do not contain a seed, a private key, or the HKDF output.
- `--org-export` of a gated org contains the landing page only. `--org-member-export --dry-run` writes nothing.

Unit tests sit beside the modules for the readability function, the signature check, the HKDF transcript, and the permission lattice. Gherkin covers the paths above. `cargo test`, `cargo clippy -- -D warnings`, and the mocked end-to-end suite pass on each minor. MSRV stays 1.88.

## 15. Delivery

Stacked draft pull requests, each based on the one above it, the first based on `main`. One minor each. The plan commits are the first commits on the first of these and do not bump the version.

| Version | Pull request | What a person can do |
| --- | --- | --- |
| none | this plan | Read the plan |
| 3.40.0 | orgs and posts | Create, edit, and delete orgs and posts. A new org is gated and private. Owner, User, and Viewer exist, and the creator is Owner |
| 3.41.0 | privacy | Public snapshot refuses every private combination, including archived rows. The per-org default can be switched |
| 3.42.0 | templates | Edit a template with a live preview. The default asks for Cinzel and Lora and stays white-label |
| 3.43.0 | assets | Add media, keep it private, and use public assets in the template. Owner removal archives an asset |
| 3.44.0 | website | Export a static site and serve it on localhost. Public rows only. An archive is an update on the stub replication log |
| 3.45.0 | service | Publish a manifest and endpoint. Another install adds it. Remote posts follow the role |
| 3.46.0 | members | Identity keys, invites, fingerprints. Create already recorded the Owner |
| 3.47.0 | roles | Custom roles between Viewer and Owner. Separate register and invite defaults. Last-owner protection. No escalation |
| 3.48.0 | gated access | Open versus Closed registration. Public and not gated: any client can view, post, reply, and attach. No identity stores `anonymous`. A signed identity is a verified author. A gated org rejects both. Owner-only actions still need a member. The browser holds a session cookie and no key |

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
8. Remote posts on a gated org. A member who posts through the service creates a post at the org's default visibility. A private result is stored and is not served. Say if those posts should be forced public. The public ungated path is the confirmed rule in section 9, not this question.
9. Two Owners. An Owner may change another Owner's role when one Owner remains. Say if that should be refused and only a new Owner added, never a demotion.
10. Fonts. Cinzel and Lora are named in CSS and not vendored. Say if the default template should ship the font files.
11. Registration and login codes last 10 minutes. The session cookie lasts 8 hours. Say if you want different lifetimes.
12. The landing page shows the org's display name plus register and login, and nothing else. Say if the name should be hidden too.
13. The service signs the registration exchange with a dedicated org key, created when the org is first published. Say if that signature should be the owner's personal identity key instead.
14. Rate limit. The plan refuses a browser session after 10 registration attempts and 10 login attempts in a minute, and the org after 30 of each in a minute. A public ungated org refuses the 11th guest write from one source address to that org in a minute, a guest body over 32 KiB, and a guest attachment over 8 MiB (confirmed 2026-10-22). Say if you want different numbers.
