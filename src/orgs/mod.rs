//! Local organizations. An org is a source that does not need a service.
//!
//! A new org is gated and private. Registration starts Closed. The creator is
//! the Owner, and the User and Viewer roles are created with the org. A public
//! page includes a row only when [`public_row`] says so. The owner's own
//! screens still show private and archived rows.

use crate::prelude::*;

pub mod access;
pub mod cli;
pub mod template;

/// Who may read a row that is being rendered, served, or exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// The person at this machine. Private rows are included.
    Owner,
    /// A public page, serve response, export, or service endpoint.
    Public,
}

/// `private` or `public`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Hidden from every public render, serve, export, and service response.
    Private,
    /// Readable when the organization is public too.
    Public,
}

impl Visibility {
    /// `private` or `public`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
        }
    }

    /// Parse a flag value.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "private" => Ok(Self::Private),
            "public" => Ok(Self::Public),
            other => bail!("visibility is private or public, not {other:?}"),
        }
    }
}

/// A post is public only when both the organization and the post are public.
/// A private organization hides every post, including ones marked public.
pub fn readable_by_others(org: Visibility, item: Visibility) -> bool {
    org == Visibility::Public && item == Visibility::Public
}

/// How new people join. Closed waits for a role that can invite. Open does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    /// A registration stays pending until someone with `invite` approves it.
    Closed,
    /// A registration becomes an active member immediately.
    Open,
}

impl Registration {
    /// `closed` or `open`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
        }
    }

    /// Parse a stored value or a flag.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "closed" => Ok(Self::Closed),
            "open" => Ok(Self::Open),
            other => bail!("registration is closed or open, not {other:?}"),
        }
    }
}

/// Whether a public render, serve, export, feed, or service response may include this post.
///
/// `member` is false for an anonymous caller. A gated org then hides the row.
/// An archived row is hidden either way. A private org hides every row.
pub fn public_row(org: &Org, post: &OrgPost, member: bool) -> bool {
    if post.archived || !readable_by_others(org.visibility, post.visibility) {
        return false;
    }
    !org.gated || member
}

/// One organization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Org {
    /// Row id.
    pub id: i64,
    /// Source id and author nick.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Optional description.
    pub description: String,
    /// Organization visibility. Private by default.
    pub visibility: Visibility,
    /// Visibility copied onto a new post that does not set its own.
    pub default_post_visibility: Visibility,
    /// Remote posts through the service. Off until that minor.
    pub allow_remote_post: bool,
    /// Service manifest published. Off until that minor.
    pub published: bool,
    /// Non-members get no content while this is on. On by default.
    pub gated: bool,
    /// Closed waits for approval. Open does not. Closed by default.
    pub registration: Registration,
    /// Role given to someone who registers. Never Owner. Viewer until changed.
    pub default_register_role: String,
    /// Role given to someone who is invited. Never Owner. Independent of registration.
    pub default_invite_role: String,
    /// When the row was created.
    pub created_at: String,
    /// When the row was last changed.
    pub updated_at: String,
}

/// One post in an organization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgPost {
    /// Stable id (`orgp-` plus 16 hex characters).
    pub id: String,
    /// Organization row id.
    pub org_id: i64,
    /// Title.
    pub title: String,
    /// Markdown body.
    pub body: String,
    /// This post's own visibility.
    pub visibility: Visibility,
    /// Ed25519 public key of the person who wrote it, hex.
    pub author_identity: String,
    /// Parent post id when this row is a reply.
    pub parent_id: Option<String>,
    /// Author until an Owner archives it, then the organization Owner.
    pub custodian_identity: String,
    /// Archived rows stay in the database and leave every public path.
    pub archived: bool,
    /// Identity that archived the row.
    pub archived_by: Option<String>,
    /// When the row was archived.
    pub archived_at: Option<String>,
    /// When the post was created.
    pub created_at: String,
    /// When the post was last changed.
    pub updated_at: String,
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS organizations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL UNIQUE COLLATE NOCASE,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private','public')),
            default_post_visibility TEXT NOT NULL DEFAULT 'private' CHECK (default_post_visibility IN ('private','public')),
            allow_remote_post INTEGER NOT NULL DEFAULT 0,
            published INTEGER NOT NULL DEFAULT 0,
            gated INTEGER NOT NULL DEFAULT 1,
            registration TEXT NOT NULL DEFAULT 'closed' CHECK (registration IN ('closed','open')),
            default_register_role TEXT NOT NULL DEFAULT 'Viewer',
            default_invite_role TEXT NOT NULL DEFAULT 'Viewer',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS org_posts (
            id TEXT PRIMARY KEY,
            org_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            visibility TEXT NOT NULL CHECK (visibility IN ('private','public')),
            author_identity TEXT NOT NULL,
            parent_id TEXT,
            custodian_identity TEXT NOT NULL,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_by TEXT,
            archived_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS org_posts_org_idx ON org_posts(org_id);
        CREATE TABLE IF NOT EXISTS org_roles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            permissions TEXT NOT NULL,
            builtin INTEGER NOT NULL DEFAULT 0,
            rank INTEGER NOT NULL,
            UNIQUE (org_id, name)
        );
        CREATE TABLE IF NOT EXISTS org_members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id INTEGER NOT NULL,
            identity_pub TEXT NOT NULL,
            role_id INTEGER NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('active','pending')),
            added_at TEXT NOT NULL,
            UNIQUE (org_id, identity_pub)
        );
        CREATE TABLE IF NOT EXISTS org_assets (
            id TEXT PRIMARY KEY,
            org_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            author_identity TEXT NOT NULL,
            visibility TEXT NOT NULL CHECK (visibility IN ('private','public')),
            custodian_identity TEXT NOT NULL,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_by TEXT,
            archived_at TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS org_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL DEFAULT 'default',
            html TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS org_write_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id INTEGER NOT NULL,
            identity_pub TEXT NOT NULL,
            nonce TEXT NOT NULL,
            guest INTEGER NOT NULL DEFAULT 0,
            source_address TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            UNIQUE (org_id, nonce)
        );";

/// Open the database and create the organization tables.
pub fn org_conn(db: &Path) -> Result<Connection> {
    let c = open_db(db)?;
    c.execute_batch("PRAGMA foreign_keys = ON;")?;
    c.execute_batch(SCHEMA)?;
    Ok(c)
}

pub(crate) fn has_table(c: &Connection, name: &str) -> bool {
    c.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

/// Slug: 1–40 characters, lowercase letters, digits, and dashes.
pub fn check_slug(raw: &str) -> Result<String> {
    let s = raw.trim();
    if !(1..=40).contains(&s.len())
        || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || s.starts_with('-')
        || s.ends_with('-')
    {
        bail!("an organization slug is 1-40 lowercase letters, digits, and dashes (got {raw:?})");
    }
    Ok(s.to_string())
}

/// Display name or title: 1–80 characters, no control characters.
pub fn check_label(raw: &str, what: &str) -> Result<String> {
    let s = raw.trim();
    if s.is_empty() || s.chars().count() > 80 || s.chars().any(char::is_control) {
        bail!("{what} needs 1-80 printable characters (got {raw:?})");
    }
    Ok(s.to_string())
}

pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn vis(raw: &str) -> Visibility {
    Visibility::parse(raw).unwrap_or(Visibility::Private)
}

fn reg(raw: &str) -> Registration {
    Registration::parse(raw).unwrap_or(Registration::Closed)
}

fn row_org(r: &rusqlite::Row) -> rusqlite::Result<Org> {
    Ok(Org {
        id: r.get(0)?,
        slug: r.get(1)?,
        name: r.get(2)?,
        description: r.get(3)?,
        visibility: vis(&r.get::<_, String>(4)?),
        default_post_visibility: vis(&r.get::<_, String>(5)?),
        allow_remote_post: r.get::<_, i64>(6)? != 0,
        published: r.get::<_, i64>(7)? != 0,
        gated: r.get::<_, i64>(8)? != 0,
        registration: reg(&r.get::<_, String>(9)?),
        default_register_role: r.get(10)?,
        default_invite_role: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

const ORG_COLS: &str = "id, slug, name, description, visibility, default_post_visibility, allow_remote_post, published, gated, registration, default_register_role, default_invite_role, created_at, updated_at";

/// Create an organization. It starts gated and private, with registration Closed.
///
/// `owner_pub` is the creator's Ed25519 public key, hex. That person becomes
/// the Owner. User and Viewer are created in the same write.
pub fn create_org(
    c: &Connection,
    slug: &str,
    name: &str,
    description: &str,
    visibility: Visibility,
    default_post_visibility: Visibility,
    owner_pub: &str,
) -> Result<Org> {
    let slug = check_slug(slug)?;
    let name = check_label(name, "an organization name")?;
    let description = description.trim();
    if description.chars().count() > 500 || description.chars().any(char::is_control) {
        bail!("a description is at most 500 printable characters");
    }
    let owner_pub = access::check_pub(owner_pub)?;
    if get_org(c, &slug)?.is_some() {
        bail!("organization {slug:?} already exists");
    }
    let t = now();
    let tx = c.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO organizations (
            slug, name, description, visibility, default_post_visibility,
            gated, registration, default_register_role, default_invite_role,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'closed', 'Viewer', 'Viewer', ?6, ?6)",
        params![slug, name, description, visibility.as_str(), default_post_visibility.as_str(), t],
    )?;
    let org_id = tx.last_insert_rowid();
    access::seed(&tx, org_id, &owner_pub)?;
    tx.commit()?;
    let org = get_org(c, &slug)?.ok_or_else(|| anyhow!("organization {slug} was not stored"))?;
    if !org.gated || org.registration != Registration::Closed {
        bail!("a new organization must start gated, with registration closed");
    }
    Ok(org)
}

/// The organization with this slug, if it exists.
pub fn get_org(c: &Connection, slug: &str) -> Result<Option<Org>> {
    if !has_table(c, "organizations") {
        return Ok(None);
    }
    let sql = format!("SELECT {ORG_COLS} FROM organizations WHERE slug = ?1 COLLATE NOCASE");
    c.query_row(&sql, params![slug.trim()], row_org).optional().map_err(Into::into)
}

/// Every organization, slug order.
pub fn list_orgs(c: &Connection) -> Result<Vec<Org>> {
    if !has_table(c, "organizations") {
        return Ok(Vec::new());
    }
    let sql = format!("SELECT {ORG_COLS} FROM organizations ORDER BY slug COLLATE NOCASE");
    let mut st = c.prepare(&sql)?;
    let v = st.query_map([], row_org)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Fields that [`update_org`] writes. `None` leaves the column alone.
pub struct OrgEdit {
    /// New display name.
    pub name: Option<String>,
    /// New description. An empty string clears it.
    pub description: Option<String>,
    /// New organization visibility.
    pub visibility: Option<Visibility>,
    /// New default for posts created after this change.
    pub default_post_visibility: Option<Visibility>,
}

/// Update the named organization.
pub fn update_org(c: &Connection, slug: &str, edit: &OrgEdit) -> Result<Org> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let name = match &edit.name {
        Some(n) => check_label(n, "an organization name")?,
        None => org.name.clone(),
    };
    let description = match &edit.description {
        Some(d) => {
            let d = d.trim();
            if d.chars().count() > 500 || d.chars().any(char::is_control) {
                bail!("a description is at most 500 printable characters");
            }
            d.to_string()
        }
        None => org.description.clone(),
    };
    let visibility = edit.visibility.unwrap_or(org.visibility);
    let default_post_visibility = edit.default_post_visibility.unwrap_or(org.default_post_visibility);
    let t = now();
    c.execute(
        "UPDATE organizations SET name = ?1, description = ?2, visibility = ?3, default_post_visibility = ?4, updated_at = ?5 WHERE id = ?6",
        params![name, description, visibility.as_str(), default_post_visibility.as_str(), t, org.id],
    )?;
    get_org(c, &org.slug)?.ok_or_else(|| anyhow!("organization {} disappeared", org.slug))
}

/// Delete the organization and its posts, roles, and members.
pub fn delete_org(c: &Connection, slug: &str) -> Result<Org> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    c.execute("DELETE FROM org_posts WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM org_assets WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM org_members WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM org_roles WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM org_templates WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM org_write_log WHERE org_id = ?1", params![org.id])?;
    c.execute("DELETE FROM organizations WHERE id = ?1", params![org.id])?;
    Ok(org)
}

/// How many posts an organization has.
pub fn post_count(c: &Connection, org_id: i64) -> Result<i64> {
    if !has_table(c, "org_posts") {
        return Ok(0);
    }
    c.query_row("SELECT COUNT(*) FROM org_posts WHERE org_id = ?1", params![org_id], |r| r.get(0)).map_err(Into::into)
}

fn new_post_id() -> String {
    let mut bytes = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    format!("orgp-{}", data_encoding::HEXLOWER.encode(&bytes))
}

fn row_post(r: &rusqlite::Row) -> rusqlite::Result<OrgPost> {
    Ok(OrgPost {
        id: r.get(0)?,
        org_id: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        visibility: vis(&r.get::<_, String>(4)?),
        author_identity: r.get(5)?,
        parent_id: r.get(6)?,
        custodian_identity: r.get(7)?,
        archived: r.get::<_, i64>(8)? != 0,
        archived_by: r.get(9)?,
        archived_at: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

const POST_COLS: &str = "id, org_id, title, body, visibility, author_identity, parent_id, custodian_identity, archived, archived_by, archived_at, created_at, updated_at";

/// Add a post written by the organization Owner. `visibility` `None` copies the organization's default.
pub fn add_post(c: &Connection, slug: &str, title: &str, body: &str, visibility: Option<Visibility>) -> Result<(Org, OrgPost)> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?} (--org-create)"))?;
    let author = access::owner_pub(c, org.id)?;
    let post = insert_post(c, &org, &author, None, title, body, visibility)?;
    Ok((org, post))
}

/// Insert a post. The author and the custodian start as the same identity.
pub(crate) fn insert_post(
    c: &Connection,
    org: &Org,
    author: &str,
    parent_id: Option<&str>,
    title: &str,
    body: &str,
    visibility: Option<Visibility>,
) -> Result<OrgPost> {
    let title = check_label(title, "a post title")?;
    if body.chars().any(char::is_control) && body.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\t') {
        bail!("a post body cannot contain control characters other than newlines and tabs");
    }
    let author = access::check_author(author)?;
    let visibility = visibility.unwrap_or(org.default_post_visibility);
    let id = new_post_id();
    let t = now();
    c.execute(
        "INSERT INTO org_posts (
            id, org_id, title, body, visibility, author_identity, parent_id,
            custodian_identity, archived, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6, 0, ?8, ?8)",
        params![id, org.id, title, body, visibility.as_str(), author, parent_id, t],
    )?;
    get_post(c, &id)?.ok_or_else(|| anyhow!("post {id} was not stored"))
}

/// One post by id.
pub fn get_post(c: &Connection, id: &str) -> Result<Option<OrgPost>> {
    if !has_table(c, "org_posts") {
        return Ok(None);
    }
    let sql = format!("SELECT {POST_COLS} FROM org_posts WHERE id = ?1");
    c.query_row(&sql, params![id], row_post).optional().map_err(Into::into)
}

/// Posts in one organization, newest first.
pub fn list_posts(c: &Connection, org_id: i64) -> Result<Vec<OrgPost>> {
    if !has_table(c, "org_posts") {
        return Ok(Vec::new());
    }
    let sql = format!("SELECT {POST_COLS} FROM org_posts WHERE org_id = ?1 ORDER BY created_at DESC");
    let mut st = c.prepare(&sql)?;
    let v = st.query_map(params![org_id], row_post)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// The stored [`Post`] shape, so the nick bar and search can show the org as the author.
pub fn as_post(org: &Org, post: &OrgPost) -> Post {
    Post {
        post_id: post.id.clone(),
        parent_post_id: String::new(),
        in_reply_to_id: String::new(),
        nick: org.slug.clone(),
        title: post.title.clone(),
        content: post.body.clone(),
        posted_at: post.created_at.clone(),
        media: Vec::new(),
        truncated: false,
        body_json: String::new(),
        score: None,
    }
}

fn kept(org: &Org, post: &OrgPost, audience: Audience) -> bool {
    match audience {
        Audience::Owner => true,
        Audience::Public => public_row(org, post, false),
    }
}

/// Text a public render, serve, export, feed, or service response may contain.
///
/// Archived rows, private rows, and rows of a gated or private org are absent.
/// The owner's local screens do not use this.
pub fn public_surface(c: &Connection) -> Result<String> {
    let mut parts = Vec::new();
    for org in list_orgs(c)? {
        for post in list_posts(c, org.id)? {
            if public_row(&org, &post, false) {
                parts.push(post.title);
                parts.push(post.body);
            }
        }
        for asset in access::list_assets(c, org.id)? {
            if access::asset_public(&org, &asset, false) {
                parts.push(asset.name);
            }
        }
    }
    Ok(parts.join("\n"))
}

/// The public JSON snapshot. Later export, serve, and service paths reuse this.
///
/// A private organization contributes no name, description, posts, or assets.
/// A public organization that is still gated contributes its name and nothing else.
/// Archived rows are left out in every case.
pub fn public_snapshot(c: &Connection, slug: &str) -> Result<Value> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let open = org.visibility == Visibility::Public && !org.gated;
    let mut posts = Vec::new();
    let mut assets = Vec::new();
    if open {
        for post in list_posts(c, org.id)? {
            if public_row(&org, &post, false) {
                posts.push(json!({
                    "id": post.id,
                    "title": post.title,
                    "body": post.body,
                    "author": post.author_identity,
                }));
            }
        }
        for asset in access::list_assets(c, org.id)? {
            if access::asset_public(&org, &asset, false) {
                assets.push(json!({
                    "id": asset.id,
                    "name": asset.name,
                    "author": asset.author_identity,
                }));
            }
        }
    }
    let mut doc = json!({
        "slug": org.slug,
        "public": open,
        "posts": posts,
        "assets": assets,
    });
    if org.visibility == Visibility::Public {
        doc["name"] = json!(org.name);
        doc["gated"] = json!(org.gated);
    }
    Ok(doc)
}

/// Organization posts as stored posts. `audience` drops rows a public page must not show.
pub fn stored_posts(c: &Connection, audience: Audience) -> Result<Vec<Post>> {
    let mut out = Vec::new();
    for org in list_orgs(c)? {
        for post in list_posts(c, org.id)? {
            if kept(&org, &post, audience) {
                out.push(as_post(&org, &post));
            }
        }
    }
    Ok(out)
}

/// One organization post by id, for the owner.
pub fn stored_post(c: &Connection, id: &str) -> Result<Option<Post>> {
    if !id.starts_with("orgp-") || !has_table(c, "org_posts") {
        return Ok(None);
    }
    let Some(post) = get_post(c, id)? else { return Ok(None) };
    let org = list_orgs(c)?.into_iter().find(|o| o.id == post.org_id);
    let Some(org) = org else { return Ok(None) };
    Ok(Some(as_post(&org, &post)))
}

/// Add organization rows to a `--list-nicks` map. Missing tables are a no-op.
pub fn merge_nicks(c: &Connection, map: &mut HashMap<String, crate::storage::db::NickInfo>) -> Result<()> {
    for org in list_orgs(c)? {
        let key = org.slug.to_lowercase();
        let count = post_count(c, org.id)?;
        let latest = list_posts(c, org.id)?.first().map(|p| p.created_at.clone());
        let e = map.entry(key).or_insert_with(|| crate::storage::db::NickInfo {
            nick: org.slug.clone(),
            display_name: None,
            post_count: 0,
            latest: None,
            profile_scraped: false,
            scraped_at: None,
        });
        if e.post_count == 0 {
            e.nick = org.slug.clone();
        }
        e.post_count += count;
        if e.display_name.is_none() {
            e.display_name = Some(org.name.clone());
        }
        if latest.as_ref().is_some_and(|t| e.latest.as_ref().is_none_or(|cur| t > cur)) {
            e.latest = latest;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> (PathBuf, Connection) {
        let d = std::env::temp_dir().join(format!(
            "acp-org-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        let c = org_conn(&d.join("db.sqlite")).unwrap();
        (d, c)
    }

    fn person() -> String {
        let secret = acp_red_star_node_core::node::NodeSecret::generate();
        data_encoding::HEXLOWER.encode(&secret.sign_pub)
    }

    #[test]
    fn a_new_org_and_post_are_private_and_a_private_org_hides_a_public_post() {
        let (_d, c) = conn();
        let org = create_org(&c, "hold", "Rebel Hold", "local", Visibility::Private, Visibility::Private, &person()).unwrap();
        assert!(org.gated);
        assert_eq!(org.registration, Registration::Closed);
        assert_eq!(org.default_register_role, "Viewer");
        assert_eq!(org.default_invite_role, "Viewer");
        assert_eq!(org.visibility, Visibility::Private);
        assert_eq!(org.default_post_visibility, Visibility::Private);
        let (_, post) = add_post(&c, "hold", "Secret", "hidden", None).unwrap();
        assert_eq!(post.visibility, Visibility::Private);
        assert!(!readable_by_others(org.visibility, post.visibility));
        let (_o, marked) = add_post(&c, "hold", "Still hidden", "no", Some(Visibility::Public)).unwrap();
        assert_eq!(marked.visibility, Visibility::Public);
        assert!(!public_row(&org, &marked, false));
        assert!(!readable_by_others(Visibility::Private, Visibility::Public));
        assert!(readable_by_others(Visibility::Public, Visibility::Public));
        assert!(!readable_by_others(Visibility::Public, Visibility::Private));
        let owner = stored_posts(&c, Audience::Owner).unwrap();
        let public = stored_posts(&c, Audience::Public).unwrap();
        assert_eq!(owner.len(), 2);
        assert!(public.is_empty());
        assert!(public_surface(&c).unwrap().is_empty());
        assert_eq!(owner[0].nick, "hold");
    }

    #[test]
    fn names_and_slugs_are_checked_and_delete_removes_posts() {
        assert!(check_slug("Hold").is_err());
        assert!(check_slug("-hold").is_err());
        assert_eq!(check_slug("hold").unwrap(), "hold");
        let (_d, c) = conn();
        create_org(&c, "hold", "Hold", "", Visibility::Private, Visibility::Private, &person()).unwrap();
        assert!(create_org(&c, "hold", "Again", "", Visibility::Private, Visibility::Private, &person()).is_err());
        add_post(&c, "hold", "One", "body", None).unwrap();
        delete_org(&c, "hold").unwrap();
        assert!(get_org(&c, "hold").unwrap().is_none());
        assert!(stored_posts(&c, Audience::Owner).unwrap().is_empty());
    }

    #[test]
    fn the_public_snapshot_refuses_private_rows_archives_and_keeps_the_old_default() {
        let (_d, c) = conn();
        let owner = person();
        let org = create_org(&c, "hold", "Rebel Hold", "secret-description", Visibility::Private, Visibility::Private, &owner).unwrap();
        add_post(&c, "hold", "Secret", "hidden-body", None).unwrap();
        let (_org, marked) = add_post(&c, "hold", "Marked", "marked-body", Some(Visibility::Public)).unwrap();
        let snap = public_snapshot(&c, "hold").unwrap();
        let text = serde_json::to_string(&snap).unwrap();
        assert!(snap["posts"].as_array().unwrap().is_empty());
        assert!(snap["assets"].as_array().unwrap().is_empty());
        assert!(!text.contains("Secret") && !text.contains("hidden-body"));
        assert!(!text.contains("Marked") && !text.contains("marked-body"));
        assert!(!text.contains("Rebel Hold") && !text.contains("secret-description"));

        update_org(&c, "hold", &OrgEdit { name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: None }).unwrap();
        let snap = public_snapshot(&c, "hold").unwrap();
        let text = serde_json::to_string(&snap).unwrap();
        assert_eq!(snap["name"].as_str(), Some("Rebel Hold"));
        assert_eq!(snap["gated"].as_bool(), Some(true));
        assert_eq!(snap["public"].as_bool(), Some(false));
        assert!(snap["posts"].as_array().unwrap().is_empty());
        assert!(!text.contains("Marked") && !text.contains("hidden-body"));

        access::set_gated(&c, org.id, false).unwrap();
        let snap = public_snapshot(&c, "hold").unwrap();
        let mut titles: Vec<_> = snap["posts"].as_array().unwrap().iter().map(|p| p["title"].as_str().unwrap()).collect();
        titles.sort_unstable();
        assert_eq!(titles, vec!["Marked"]);
        assert!(!serde_json::to_string(&snap).unwrap().contains("hidden-body"));

        update_org(&c, "hold", &OrgEdit { name: None, description: None, visibility: None, default_post_visibility: Some(Visibility::Public) }).unwrap();
        let secret = list_posts(&c, org.id).unwrap().into_iter().find(|p| p.title == "Secret").unwrap();
        assert_eq!(secret.visibility, Visibility::Private);
        let (_org, later) = add_post(&c, "hold", "Later", "after-switch", None).unwrap();
        assert_eq!(later.visibility, Visibility::Public);
        let snap = public_snapshot(&c, "hold").unwrap();
        let mut titles: Vec<_> = snap["posts"].as_array().unwrap().iter().map(|p| p["title"].as_str().unwrap()).collect();
        titles.sort_unstable();
        assert_eq!(titles, vec!["Later", "Marked"]);
        assert!(!serde_json::to_string(&snap).unwrap().contains("hidden-body"));

        let asset = access::submit_guest_attachment(&c, "hold", "203.0.113.9", &later.id, "banner-file", 12, Some(Visibility::Public)).unwrap();
        assert!(serde_json::to_string(&public_snapshot(&c, "hold").unwrap()).unwrap().contains("banner-file"));
        let record = access::archive_asset(&c, org.id, &owner, &asset.id).unwrap();
        assert_eq!(record.kind, "update");
        assert!(!serde_json::to_string(&public_snapshot(&c, "hold").unwrap()).unwrap().contains("banner-file"));

        let record = access::archive_post(&c, org.id, &owner, &marked.id).unwrap();
        assert_eq!(record.kind, "update");
        let snap = public_snapshot(&c, "hold").unwrap();
        let text = serde_json::to_string(&snap).unwrap();
        assert!(!text.contains("Marked") && !text.contains("marked-body"));
        assert!(text.contains("Later"));
        assert!(get_post(&c, &marked.id).unwrap().unwrap().archived);
    }
}
