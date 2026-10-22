# Plan: Organizations

Status: **plan, 2026-10-22.** Nothing in this document is implemented yet. It is the first commit on the organizations draft. The implementation commits follow it, one minor at a time.

Base: `main` at `274a9e01` (crate `3.39.0`). This plan does not bump the version. `AGENTS.md` bumps a version for a feature or a fix, not for a plan.

An organization is a local content source. It does not need the ACP Red Star service package, and it does not talk to acp.us. Its public face is a website: a template, pages, a feed, and downloads. Another Red Star install can add that website as a service. Private content stays on the machine that owns it.

The core stays white-label. The built-in Red Star theme can still supply the default look. A `plain` theme, or any other theme, can replace it. Federation, replication, and the network transport are stubs. They get no version.

## 1. Overview and goals

- A person can create an organization on this machine and post in it with no service installed.
- The org is a source. Its posts show up wherever source content shows up, with the org as the author.
- Visibility is private unless someone deliberately makes it public. The same rule covers the org, each post, and each asset.
- A public org, rendered through its template, is a static site: index, post pages, asset and download pages, and a feed. Any node can export that site to a folder or serve it.
- A public org can publish itself as a service. Another install adds it and browses it like any other source. Posting into it is allowed only when the org says so and the caller's role allows it.
- Members are Red Star users with their own identity keys. Invites are an authenticated Diffie-Hellman exchange. Roles decide what a member can change.
- The person who creates an org is its owner. The org starts with that one owner. The last owner cannot be removed.
- Private material never appears in a public render, a serve response, an export, or the service endpoint.

What the owner sees in the terminal UI and in `--org-show` is the local copy, including private rows. That is not a public render. Every path that writes a page, a file for someone else, or a network response uses the public filter in section 3.

## 2. Organizations

Create, edit, and delete from the command line and from the terminal UI.

- The creator's identity is added as a member with the built-in owner role at the moment the org is created. That lands with members (3.46.0), because that is when an identity key exists. Orgs created by the earlier minor are backfilled: the first time 3.46.0 opens the database, any org with no members gets the local identity as its owner.
- An org has a slug (the source id) and a display name. The slug is the author nick on posts.
- Description is optional.
- Visibility is `private` or `public`. The default is `private`.
- Each org has a default for new posts, also `private` until switched.
- Delete removes the org, its posts, its assets, its templates, its invites, and its memberships. It asks first. Cancel is the default.

Making an org public is a live exposure. The command prints what would become readable, then asks. `--dry-run` prints that list and writes nothing. `--yes` confirms. Without a terminal, and without `--yes`, the change is refused.

## 3. Posts

Posts are rows in the local database, not files beside it. The org slug is the source and the author.

- Each post has its own visibility. The default is the org's default, which starts as `private`.
- Switching the org default to `public` does not rewrite posts already stored. It only applies to posts created after the switch. A person can still mark one new post private.
- Readability for anyone who is not looking at the owner's local screens:
  - A public org is readable, except posts (and assets) that are themselves private.
  - A private org is never readable. A post marked public inside a private org stays hidden.
- The owner's nick bar, search, and `--org-show` include private posts. They are labeled private.
- `--export`, the site folder, `serve`, and the service endpoint call one function, `readable_by_others`. They do not grow their own filters.

## 4. Members and identity

A member is a Red Star user. Each user has a long-term identity, generated on first use:

- Ed25519 for signing (`ed25519-dalek`).
- X25519 for Diffie-Hellman (`x25519-dalek`).
- Both seeds are sealed at rest (section 11). Nothing here hand-rolls a curve or a hash.

Adding a user is an authenticated exchange. Each side's X25519 public value is signed by that side's Ed25519 identity key, so a substituted public key fails verification.

1. The inviter writes an invite: org slug, role, expiry, a single-use id, the inviter's Ed25519 public key, the inviter's X25519 public key, and the signature over those fields.
2. The invitee checks the signature, answers with their own Ed25519 public key, X25519 public key, and signature.
3. Both sides run X25519, then HKDF-SHA256 (`hkdf` 0.12, already in the tree) with a fixed info string and the transcript (invite id and both public keys). The output is not shown.
4. Both sides show the same short fingerprint of that transcript. The person compares the two screens out of band and types the code. If the typed code does not match the computed code, the member is not added.
5. The inviter marks the invite used. A second accept of the same invite is refused. An invite past its expiry is refused.

Local transport, in the mocked tests, is a file the two throwaway `HOME`s share, or a localhost socket bound to `127.0.0.1`. The decentralized transport is a stub with no version.

Invites do not contain private keys. Logs do not contain seeds, shared secrets, or the HKDF output.

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
| `invite` | Create an invite |
| `remove_members` | Remove a member who is not the last owner |
| `manage_roles` | Create, edit, and delete custom roles, and assign a role |

Built-in roles, shipped with every org:

| Role | Permissions |
| --- | --- |
| owner | All of the above, and the only role that may assign `owner` |
| admin | All except assigning `owner`. An admin cannot change an owner's role or remove an owner |
| editor | `post`, `edit_own`, `edit_any`, `delete_own`, `assets`, `templates` |
| member | `post`, `edit_own`, `delete_own` |
| read-only | None of the write permissions |

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

## 8. Website

A public org rendered through its template is a complete static site:

- `index.html`, the front page.
- One HTML page per public post.
- One download page per public asset, plus the file bytes.
- `feed.json`, and a Markdown file per public post. JSON and HTML are the core's export formats today. Markdown is written as text files beside them. Whether `--format` grows an `md` value is an open question (section 15).
- `--org-export SLUG DIR` writes that tree. A private org writes an empty public site and says so. Private posts and private assets are counted in the summary and left out of the files.
- `--org-serve SLUG` binds `127.0.0.1` by default and serves that same public tree. A private org does not serve content.

Any later node that can read the public bundle can serve it. That is the "every node can serve" rule. The code that would replicate the bundle onto another node is the stub in section 10.

## 9. Organization as a service

Publishing is off by default.

A public org can publish a service manifest and a localhost endpoint. The manifest is the core plugin record: a service id, a display name, the endpoint, and the capabilities the org is willing to offer (`reader`, and `post` only when remote posts are allowed). It is stored as `~/.acp/services/org-<slug>.toml`. The endpoint speaks the same records the plugin crate already has (`Page`, `ItemRec`, `Draft`).

Another install adds it with `--org-add URL`. That install then browses the org like any other source. The read goes through `Reader` and the readiness model (`ready`, `off`, `gated`, and the rest). `Post` is `off` until the org allows remote posts. A remote post is checked against the caller's role (section 5). Only public posts and public assets are returned.

Publishing, withdrawing, and turning remote posts on are live writes. The preview lists the public posts and public assets that would be exposed, names the capabilities, and asks. Cancel is the default. `--dry-run` prints the manifest and does not write it and does not listen. `--yes` confirms.

The in-process adapter implements the existing `Service`, `Reader`, and `Post` traits. It is not registered as a built-in at startup. This binary still ships with no built-in site service. The adapter is loaded only after a person adds the manifest.

## 10. Network

Every node that has the public bundle can serve it in the formats section 8 lists. That serving is local in this stack (`127.0.0.1` unless a later, explicit setting says otherwise).

These are stubs. They are not features. They get no version. The first release that has a place to mention them records them under Added as `prepared, gated: …`:

- Federation and replication of org content between nodes.
- The decentralized transport for invites and for the service endpoint. Files and localhost are the transports that actually run.

No packet is sent to acp.us. Tests use a throwaway `HOME` and, where two installs are required, two throwaway homes.

## 11. Security and privacy

- `readable_by_others` is the only public filter. Render, serve, export, and the service call it. Tests cover every combination: private org, public org with a private post, public org whose default is public, and private assets.
- Identity seeds are sealed with XChaCha20-Poly1305 under a key derived from the unlocked database key, the same construction as the node keystore (`keystore_key` in `acp_red_star_node_core`). The file mode is `0600`. The directory mode is `0700`.
- Seeds, shared secrets, and the HKDF output never appear in logs, exports, HTML, serve bodies, service JSON, or test snapshots. `Debug` for the key type prints `[redacted]`.
- An invite file holds public keys and signatures only.
- Confirmations that expose data (public visibility, publish, remote post) default to Cancel.
- Deletes default to Cancel.
- A fingerprint mismatch, an expired invite, and a replayed invite add nobody.

## 12. Data model, commands, and screens

Tables, created on first use in the encrypted database. Names can still move if a migration is cleaner. The columns are the contract.

```text
organizations
  id, slug unique, name, description,
  visibility,            -- private | public, default private
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
  org_id, identity_pub, role_id, added_at

org_invites
  id, org_id, role_id, inviter_pub,
  expires_at, used_at, nonce
```

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

Terminal UI. All of it is styled with ratatui-lipgloss. Each screen has a tooltip and a Help topic.

- Library › Organizations. List, create, edit, delete. The create form shows that the creator becomes the owner.
- The nick bar gains the org as a source. Opening it lists that org's posts. Private rows are marked.
- Organizations › Template. Source and live preview, side by side. The preview uses the theme on screen.
- Organizations › Assets.
- Organizations › Members. Roster, invite, accept, and the fingerprint prompt.
- Organizations › Roles. Built-in roles, custom roles, assignments. The last owner is marked and cannot be cleared from this screen.
- A confirm modal, Cancel focused, for making something public, publishing, deleting, and removing a member.

Help topics: `orgs`, `org-posts`, `org-templates`, `org-assets`, `org-serve`, `org-service`, `org-members`, `org-roles`.

## 13. Test plan

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

Unit tests sit beside the modules for the readability function, the signature check, the HKDF transcript, and the permission lattice. Gherkin covers the paths above. `cargo test`, `cargo clippy -- -D warnings`, and the mocked end-to-end suite pass on each minor. MSRV stays 1.88.

## 14. Delivery

Stacked draft pull requests, each based on the one above it, the first based on `main`. One minor each. The plan commit is the first commit on the first of these and does not bump the version.

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

Stubs, with no version of their own, recorded as `prepared, gated:` on the release that first has somewhere to put the sentence (3.44.0 for replication, 3.46.0 for invite transport):

- Federation and replication of org content.
- Any transport that is not a file or `127.0.0.1`.

Plugin crate version stays where `main` left it. These minors are the root crate.

## 15. Open questions

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
