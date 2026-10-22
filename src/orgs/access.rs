//! Roles, registration, and the public-ungated guest path.
//!
//! Owner, User, and Viewer are created with the organization. Custom roles sit
//! strictly between Viewer and Owner. A guest may write on a public org that is
//! not gated, after an Ed25519 signature, and cannot remove anyone else's post.

use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::time::Duration;

use acp_red_star_node_core::node::{NodeSecret, keystore_key};
use acp_red_star_node_core::store::{open_blob, seal_blob};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use zeroize::Zeroizing;

use super::{Org, OrgPost, Registration, Visibility, check_label, get_post, insert_post, now};
use crate::prelude::*;

/// Permissions this build knows. Owner allows every one of them, and any later
/// check, because the set is meant to grow.
pub const PERMISSIONS: &[&str] = &[
    "view",
    "post",
    "reply",
    "attach",
    "edit_own",
    "edit_any",
    "archive_own",
    "archive_any",
    "post_visibility",
    "org_visibility",
    "assets",
    "templates",
    "publish",
    "invite",
    "remove_members",
    "manage_roles",
    "manage_access",
];

/// Only an Owner may put these on a custom role.
const OWNER_ONLY: &[&str] = &["archive_any", "remove_members"];

const RANK_VIEWER: i64 = 0;
const RANK_USER: i64 = 500;
const RANK_OWNER: i64 = 1000;

/// Guest writes allowed from one source address, in one org, per minute.
pub const GUEST_WRITES_PER_MINUTE: i64 = 10;
/// Largest body a non-member may store on a public org that is not gated.
pub const GUEST_BODY_MAX: usize = 32 * 1024;
/// Largest attachment a non-member may store on a public org that is not gated.
pub const GUEST_ATTACHMENT_MAX: u64 = 8 * 1024 * 1024;
/// Author stored when the client presented no Red Star identity.
pub const ANONYMOUS_AUTHOR: &str = "anonymous";

const IDENTITY_AAD: &[u8] = b"acp-org-identity-v1";

/// Which default a join uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    /// Someone registering themselves.
    Register,
    /// Someone invited by a member who can invite.
    Invite,
}

/// `active` or `pending`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberStatus {
    /// The person can use the role.
    Active,
    /// Closed registration, waiting for approval.
    Pending,
}

impl MemberStatus {
    /// `active` or `pending`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Pending => "pending",
        }
    }

    fn parse(raw: &str) -> Self {
        if raw == "active" { Self::Active } else { Self::Pending }
    }
}

/// One role in an organization's hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    /// Row id.
    pub id: i64,
    /// Organization row id.
    pub org_id: i64,
    /// Display name. Built-ins are Owner, User, and Viewer.
    pub name: String,
    /// Space-separated permission names. Owner is checked as every permission.
    pub permissions: String,
    /// Created with the organization. These three cannot be deleted.
    pub builtin: bool,
    /// Viewer is 0, User is 500, Owner is 1000. Custom ranks are 1 through 999.
    pub rank: i64,
}

/// One member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// Organization row id.
    pub org_id: i64,
    /// Ed25519 public key, hex.
    pub identity_pub: String,
    /// Role name.
    pub role: String,
    /// Role rank.
    pub rank: i64,
    /// Active or pending.
    pub status: MemberStatus,
}

/// A file kept with an organization. The bytes live beside the database, mode `0600`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// Stable id (`orga-` plus 16 hex characters).
    pub id: String,
    /// Organization row id.
    pub org_id: i64,
    /// File name shown to readers.
    pub name: String,
    /// Who uploaded it.
    pub author_identity: String,
    /// This asset's own visibility. Private until someone makes it public.
    pub visibility: Visibility,
    /// Author until an Owner archives it, then the organization Owner.
    pub custodian_identity: String,
    /// Archived assets stay stored and leave every public path.
    pub archived: bool,
    /// Identity that archived the asset.
    pub archived_by: Option<String>,
    /// When the asset was archived.
    pub archived_at: Option<String>,
    /// Path relative to the home directory. Empty when no bytes were stored.
    pub path: String,
    /// Media type. Empty when the file was not stored.
    pub mime: String,
}

/// What replication carries when an Owner archives a row. There is no delete kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicationUpdate {
    /// Always `update`.
    pub kind: &'static str,
    /// The row is archived.
    pub archived: bool,
    /// `private` after an Owner removal.
    pub visibility: &'static str,
    /// The organization Owner's public key.
    pub custodian_identity: String,
    /// The original author. Unchanged.
    pub author_identity: String,
    /// Who archived it.
    pub archived_by: String,
    /// When it was archived.
    pub archived_at: String,
}

/// Canonical bytes a guest or member signs for a new post.
pub fn post_message(nonce: &str, slug: &str, title: &str, body: &str) -> Vec<u8> {
    format!("acp-org-post-v1\n{nonce}\n{slug}\n{title}\n{body}").into_bytes()
}

/// Canonical bytes for a reply.
pub fn reply_message(nonce: &str, slug: &str, parent_id: &str, title: &str, body: &str) -> Vec<u8> {
    format!("acp-org-reply-v1\n{nonce}\n{slug}\n{parent_id}\n{title}\n{body}").into_bytes()
}

/// Canonical bytes for an attachment name on a post.
pub fn attach_message(nonce: &str, slug: &str, post_id: &str, name: &str) -> Vec<u8> {
    format!("acp-org-attach-v1\n{nonce}\n{slug}\n{post_id}\n{name}").into_bytes()
}

/// A public key, or [`ANONYMOUS_AUTHOR`] when the client did not sign.
pub fn check_author(raw: &str) -> Result<String> {
    if raw == ANONYMOUS_AUTHOR {
        return Ok(ANONYMOUS_AUTHOR.to_string());
    }
    check_pub(raw)
}

/// 32-byte Ed25519 public key, lowercase hex.
pub fn check_pub(raw: &str) -> Result<String> {
    let decoded = data_encoding::HEXLOWER.decode(raw.trim().to_ascii_lowercase().as_bytes()).map_err(|_| anyhow!("an identity key is 32 bytes of hex"))?;
    if decoded.len() != 32 {
        bail!("an identity key is 32 bytes of hex");
    }
    Ok(data_encoding::HEXLOWER.encode(&decoded))
}

/// Lowercase hex of an Ed25519 public key.
pub fn pub_hex(secret: &NodeSecret) -> String {
    data_encoding::HEXLOWER.encode(&secret.sign_pub)
}

fn identity_path(db: &Path) -> PathBuf {
    db.parent().unwrap_or_else(|| Path::new(".")).join("org-identity.key")
}

/// The local organization's signing key, sealed next to the database.
///
/// The file is created the first time, mode `0600`, under `keystore_key`.
pub fn local_identity_pub(db: &Path) -> Result<String> {
    let path = identity_path(db);
    let key = crate::crypto::atrest::cached_key(db).ok_or_else(|| anyhow!("unlock the database before creating an organization"))?;
    if path.is_file() {
        return Ok(pub_hex(&load_identity(&path, &key)?));
    }
    let secret = NodeSecret::generate();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    store_identity(&path, &secret, &key)?;
    Ok(pub_hex(&secret))
}

/// Seal both seeds. Debug output of [`NodeSecret`] stays `[redacted]`.
pub fn store_identity(path: &Path, secret: &NodeSecret, local_key: &[u8; 32]) -> Result<()> {
    let key = keystore_key(local_key);
    let mut plain = Zeroizing::new([0u8; 64]);
    plain[..32].copy_from_slice(secret.sign_seed());
    plain[32..].copy_from_slice(secret.enc_seed());
    let blob = seal_blob(&key, IDENTITY_AAD, plain.as_ref())?;
    write_private(path, &blob)
}

/// Open a file written by [`store_identity`].
pub fn load_identity(path: &Path, local_key: &[u8; 32]) -> Result<NodeSecret> {
    let key = keystore_key(local_key);
    let blob = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let plain = open_blob(&key, IDENTITY_AAD, &blob)?;
    if plain.len() != 64 {
        bail!("organization identity file is damaged");
    }
    let mut sign = [0u8; 32];
    let mut enc = [0u8; 32];
    sign.copy_from_slice(&plain[..32]);
    enc.copy_from_slice(&plain[32..]);
    Ok(NodeSecret::from_seeds(sign, enc))
}

pub(crate) fn write_private(path: &Path, body: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(body)?;
    Ok(())
}

/// Insert Owner, User, and Viewer, and the creator as an active Owner.
pub fn seed(c: &Connection, org_id: i64, owner_pub: &str) -> Result<()> {
    let owner_pub = check_pub(owner_pub)?;
    let owner_id = insert_role(c, org_id, "Owner", &PERMISSIONS.join(" "), true, RANK_OWNER)?;
    insert_role(c, org_id, "User", "view post reply attach", true, RANK_USER)?;
    insert_role(c, org_id, "Viewer", "view", true, RANK_VIEWER)?;
    c.execute(
        "INSERT INTO org_members (org_id, identity_pub, role_id, status, added_at) VALUES (?1, ?2, ?3, 'active', ?4)",
        params![org_id, owner_pub, owner_id, now()],
    )?;
    Ok(())
}

fn insert_role(c: &Connection, org_id: i64, name: &str, permissions: &str, builtin: bool, rank: i64) -> Result<i64> {
    c.execute(
        "INSERT INTO org_roles (org_id, name, permissions, builtin, rank) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![org_id, name, permissions, i64::from(builtin), rank],
    )?;
    Ok(c.last_insert_rowid())
}

fn row_role(r: &rusqlite::Row) -> rusqlite::Result<Role> {
    Ok(Role {
        id: r.get(0)?,
        org_id: r.get(1)?,
        name: r.get(2)?,
        permissions: r.get(3)?,
        builtin: r.get::<_, i64>(4)? != 0,
        rank: r.get(5)?,
    })
}

/// Roles, highest rank first.
pub fn roles(c: &Connection, org_id: i64) -> Result<Vec<Role>> {
    let mut st = c.prepare("SELECT id, org_id, name, permissions, builtin, rank FROM org_roles WHERE org_id = ?1 ORDER BY rank DESC, name COLLATE NOCASE")?;
    Ok(st.query_map(params![org_id], row_role)?.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn role_named(c: &Connection, org_id: i64, name: &str) -> Result<Role> {
    roles(c, org_id)?
        .into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("no role {name:?}"))
}

/// Members, oldest first.
pub fn members(c: &Connection, org_id: i64) -> Result<Vec<Member>> {
    let mut st = c.prepare(
        "SELECT m.org_id, m.identity_pub, r.name, r.rank, m.status
         FROM org_members m JOIN org_roles r ON r.id = m.role_id
         WHERE m.org_id = ?1 ORDER BY m.added_at, m.id",
    )?;
    let v = st
        .query_map(params![org_id], |r| {
            Ok(Member {
                org_id: r.get(0)?,
                identity_pub: r.get(1)?,
                role: r.get(2)?,
                rank: r.get(3)?,
                status: MemberStatus::parse(&r.get::<_, String>(4)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// The organization Owner's public key. The earliest Owner wins when there are two.
pub fn owner_pub(c: &Connection, org_id: i64) -> Result<String> {
    members(c, org_id)?
        .into_iter()
        .find(|m| m.role.eq_ignore_ascii_case("Owner") && m.status == MemberStatus::Active)
        .map(|m| m.identity_pub)
        .ok_or_else(|| anyhow!("organization has no owner"))
}

fn member_of(c: &Connection, org_id: i64, identity: &str) -> Result<Option<Member>> {
    let identity = check_pub(identity)?;
    Ok(members(c, org_id)?.into_iter().find(|m| m.identity_pub == identity))
}

fn is_owner(role_name: &str) -> bool {
    role_name.eq_ignore_ascii_case("Owner")
}

fn allows(role: &Role, permission: &str) -> bool {
    if role.builtin && is_owner(&role.name) {
        return true;
    }
    role.permissions.split_whitespace().any(|p| p == permission)
}

fn actor_role(c: &Connection, org_id: i64, actor: &str) -> Result<Role> {
    let member = member_of(c, org_id, actor)?.ok_or_else(|| anyhow!("not a member"))?;
    if member.status != MemberStatus::Active {
        bail!("membership is still pending");
    }
    role_named(c, org_id, &member.role)
}

pub(crate) fn require(c: &Connection, org_id: i64, actor: &str, permission: &str) -> Result<Role> {
    let role = actor_role(c, org_id, actor)?;
    if !allows(&role, permission) {
        bail!("missing permission {permission}");
    }
    Ok(role)
}

/// Turn gating on or off. Content stays private until visibility changes too.
pub fn set_gated(c: &Connection, org_id: i64, gated: bool) -> Result<()> {
    c.execute("UPDATE organizations SET gated = ?1, updated_at = ?2 WHERE id = ?3", params![i64::from(gated), now(), org_id])?;
    Ok(())
}

/// `closed` or `open`.
pub fn set_registration(c: &Connection, org_id: i64, registration: Registration) -> Result<()> {
    c.execute(
        "UPDATE organizations SET registration = ?1, updated_at = ?2 WHERE id = ?3",
        params![registration.as_str(), now(), org_id],
    )?;
    Ok(())
}

/// Change the role new members get. Owner cannot be chosen. The two joins are independent.
pub fn set_default_role(c: &Connection, org_id: i64, join: Join, role_name: &str) -> Result<()> {
    let role = role_named(c, org_id, role_name)?;
    if is_owner(&role.name) {
        bail!("the Owner role cannot be a default");
    }
    let column = match join {
        Join::Register => "default_register_role",
        Join::Invite => "default_invite_role",
    };
    c.execute(&format!("UPDATE organizations SET {column} = ?1, updated_at = ?2 WHERE id = ?3"), params![role.name, now(), org_id])?;
    Ok(())
}

fn org_by_id(c: &Connection, org_id: i64) -> Result<Org> {
    super::list_orgs(c)?.into_iter().find(|o| o.id == org_id).ok_or_else(|| anyhow!("no organization"))
}

fn add_member(c: &Connection, org_id: i64, identity: &str, role: &Role, status: MemberStatus) -> Result<Member> {
    let identity = check_pub(identity)?;
    if member_of(c, org_id, &identity)?.is_some() {
        bail!("already a member");
    }
    c.execute(
        "INSERT INTO org_members (org_id, identity_pub, role_id, status, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![org_id, identity, role.id, status.as_str(), now()],
    )?;
    member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("member was not stored"))
}

/// Register. Closed leaves the member pending. Open activates them. The role is the registration default.
pub fn register_member(c: &Connection, org_id: i64, identity: &str) -> Result<Member> {
    let org = org_by_id(c, org_id)?;
    let role = role_named(c, org_id, &org.default_register_role)?;
    let status = match org.registration {
        Registration::Closed => MemberStatus::Pending,
        Registration::Open => MemberStatus::Active,
    };
    add_member(c, org_id, identity, &role, status)
}

/// Invite. The member is active, at the invitation default. The actor needs `invite`.
pub fn invite_member(c: &Connection, org_id: i64, actor: &str, identity: &str) -> Result<Member> {
    require(c, org_id, actor, "invite")?;
    let org = org_by_id(c, org_id)?;
    let role = role_named(c, org_id, &org.default_invite_role)?;
    add_member(c, org_id, identity, &role, MemberStatus::Active)
}

/// Approve a pending registration. The actor needs `invite`. The role stays as it was.
pub fn approve_registration(c: &Connection, org_id: i64, actor: &str, identity: &str) -> Result<Member> {
    require(c, org_id, actor, "invite")?;
    let identity = check_pub(identity)?;
    let member = member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("no registration {identity}"))?;
    if member.status != MemberStatus::Pending {
        bail!("registration is not pending");
    }
    c.execute(
        "UPDATE org_members SET status = 'active' WHERE org_id = ?1 AND identity_pub = ?2",
        params![org_id, identity],
    )?;
    member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("member disappeared"))
}

fn owner_count(c: &Connection, org_id: i64) -> Result<i64> {
    Ok(members(c, org_id)?.iter().filter(|m| is_owner(&m.role) && m.status == MemberStatus::Active).count() as i64)
}

/// Give a member another role.
///
/// A person who is not Owner cannot grant a role at or above their own rank,
/// and cannot change someone already at or above that rank. The last Owner stays.
pub fn assign_role(c: &Connection, org_id: i64, actor: &str, identity: &str, role_name: &str) -> Result<Member> {
    let actor_role = require(c, org_id, actor, "manage_roles")?;
    let identity = check_pub(identity)?;
    let subject = member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("no member {identity}"))?;
    let next = role_named(c, org_id, role_name)?;
    if !is_owner(&actor_role.name) && (subject.rank >= actor_role.rank || next.rank >= actor_role.rank) {
        bail!("cannot grant a role at or above your own");
    }
    if is_owner(&subject.role) && !is_owner(&next.name) && owner_count(c, org_id)? <= 1 {
        bail!("the last owner stays");
    }
    c.execute(
        "UPDATE org_members SET role_id = ?1 WHERE org_id = ?2 AND identity_pub = ?3",
        params![next.id, org_id, identity],
    )?;
    member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("member disappeared"))
}

/// Add a custom role between Viewer and Owner. Built-in names are reserved.
pub fn create_custom_role(c: &Connection, org_id: i64, actor: &str, name: &str, rank: i64, permissions: &[&str]) -> Result<Role> {
    let actor_role = require(c, org_id, actor, "manage_roles")?;
    let name = check_label(name, "a role name")?;
    if name.eq_ignore_ascii_case("Owner") || name.eq_ignore_ascii_case("User") || name.eq_ignore_ascii_case("Viewer") {
        bail!("built-in roles stay");
    }
    if !(1..=999).contains(&rank) {
        bail!("a custom role rank is 1 through 999");
    }
    if !is_owner(&actor_role.name) && rank >= actor_role.rank {
        bail!("cannot grant a role at or above your own");
    }
    if permissions.is_empty() {
        bail!("a role needs a permission");
    }
    let mut unique = Vec::new();
    for perm in permissions {
        if !PERMISSIONS.contains(perm) {
            bail!("unknown permission {perm}");
        }
        if OWNER_ONLY.contains(perm) && !is_owner(&actor_role.name) {
            bail!("only an Owner can grant {perm}");
        }
        if !allows(&actor_role, perm) {
            bail!("missing permission {perm}");
        }
        if !unique.contains(perm) {
            unique.push(*perm);
        }
    }
    insert_role(c, org_id, &name, &unique.join(" "), false, rank)?;
    role_named(c, org_id, &name)
}

/// Delete a custom role that nobody holds. Built-in roles stay.
pub fn delete_role(c: &Connection, org_id: i64, actor: &str, name: &str) -> Result<()> {
    require(c, org_id, actor, "manage_roles")?;
    let role = role_named(c, org_id, name)?;
    if role.builtin {
        bail!("built-in roles stay");
    }
    let held = members(c, org_id)?.iter().any(|m| m.role.eq_ignore_ascii_case(&role.name));
    if held {
        bail!("that role still has members");
    }
    c.execute("DELETE FROM org_roles WHERE id = ?1", params![role.id])?;
    Ok(())
}

/// Remove a member. Their posts stay. The last Owner stays.
pub fn remove_member(c: &Connection, org_id: i64, actor: &str, identity: &str) -> Result<()> {
    require(c, org_id, actor, "remove_members")?;
    let identity = check_pub(identity)?;
    let subject = member_of(c, org_id, &identity)?.ok_or_else(|| anyhow!("no member {identity}"))?;
    if is_owner(&subject.role) && owner_count(c, org_id)? <= 1 {
        bail!("the last owner stays");
    }
    c.execute("DELETE FROM org_members WHERE org_id = ?1 AND identity_pub = ?2", params![org_id, identity])?;
    Ok(())
}

fn finish_archive(author: &str, custodian: &str, actor: &str, at: &str) -> ReplicationUpdate {
    ReplicationUpdate {
        kind: "update",
        archived: true,
        visibility: "private",
        custodian_identity: custodian.to_string(),
        author_identity: author.to_string(),
        archived_by: actor.to_string(),
        archived_at: at.to_string(),
    }
}

/// Archive another person's post. The row stays. Custody moves to the organization Owner.
pub fn archive_post(c: &Connection, org_id: i64, actor: &str, post_id: &str) -> Result<ReplicationUpdate> {
    let actor_pub = check_pub(actor)?;
    require(c, org_id, &actor_pub, "archive_any")?;
    let post = get_post(c, post_id)?.ok_or_else(|| anyhow!("no post {post_id}"))?;
    if post.org_id != org_id {
        bail!("no post {post_id}");
    }
    let custodian = owner_pub(c, org_id)?;
    let at = now();
    c.execute(
        "UPDATE org_posts SET archived = 1, visibility = 'private', custodian_identity = ?1, archived_by = ?2, archived_at = ?3, updated_at = ?3 WHERE id = ?4",
        params![custodian, actor_pub, at, post.id],
    )?;
    Ok(finish_archive(&post.author_identity, &custodian, &actor_pub, &at))
}

/// Archive another person's asset. Same custody rule as [`archive_post`].
pub fn archive_asset(c: &Connection, org_id: i64, actor: &str, asset_id: &str) -> Result<ReplicationUpdate> {
    let actor_pub = check_pub(actor)?;
    require(c, org_id, &actor_pub, "archive_any")?;
    let asset = list_assets(c, org_id)?.into_iter().find(|a| a.id == asset_id).ok_or_else(|| anyhow!("no asset {asset_id}"))?;
    let custodian = owner_pub(c, org_id)?;
    let at = now();
    c.execute(
        "UPDATE org_assets SET archived = 1, visibility = 'private', custodian_identity = ?1, archived_by = ?2, archived_at = ?3 WHERE id = ?4",
        params![custodian, actor_pub, at, asset.id],
    )?;
    Ok(finish_archive(&asset.author_identity, &custodian, &actor_pub, &at))
}

fn row_asset(r: &rusqlite::Row) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: r.get(0)?,
        org_id: r.get(1)?,
        name: r.get(2)?,
        author_identity: r.get(3)?,
        visibility: Visibility::parse(&r.get::<_, String>(4)?).unwrap_or(Visibility::Private),
        custodian_identity: r.get(5)?,
        archived: r.get::<_, i64>(6)? != 0,
        archived_by: r.get(7)?,
        archived_at: r.get(8)?,
        path: r.get(9)?,
        mime: r.get(10)?,
    })
}

/// Assets in one organization.
pub fn list_assets(c: &Connection, org_id: i64) -> Result<Vec<Asset>> {
    if super::has_table(c, "org_assets") {
        let mut st = c.prepare(
            "SELECT id, org_id, name, author_identity, visibility, custodian_identity, archived, archived_by, archived_at, path, mime FROM org_assets WHERE org_id = ?1 ORDER BY created_at",
        )?;
        return Ok(st.query_map(params![org_id], row_asset)?.collect::<rusqlite::Result<Vec<_>>>()?);
    }
    Ok(Vec::new())
}

/// Whether a public path may name this asset. Same rule as [`super::public_row`].
pub fn asset_public(org: &Org, asset: &Asset, member: bool) -> bool {
    if asset.archived || !super::readable_by_others(org.visibility, asset.visibility) {
        return false;
    }
    !org.gated || member
}

fn verify(identity: &str, message: &[u8], signature: &[u8]) -> Result<()> {
    if signature.len() != 64 {
        bail!("signature rejected");
    }
    let bytes = data_encoding::HEXLOWER.decode(identity.as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    let key_bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| anyhow!("signature rejected"))?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| anyhow!("signature rejected"))?;
    let mut sig = [0u8; 64];
    sig.copy_from_slice(signature);
    key.verify(message, &Signature::from_bytes(&sig)).map_err(|_| anyhow!("signature rejected"))?;
    Ok(())
}

fn check_nonce(nonce: &str) -> Result<()> {
    if nonce.len() < 16 || nonce.len() > 64 || !nonce.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("a nonce is 16 to 64 hex characters");
    }
    Ok(())
}

fn check_source(raw: &str) -> Result<String> {
    let source = raw.trim();
    if source.is_empty() || source.chars().count() > 200 || source.chars().any(char::is_control) {
        bail!("a client address is 1-200 printable characters");
    }
    Ok(source.to_string())
}

fn check_guest_body(body: &str) -> Result<()> {
    if body.len() > GUEST_BODY_MAX {
        bail!("guest body is too large");
    }
    Ok(())
}

fn check_guest_attachment(byte_len: u64) -> Result<()> {
    if byte_len > GUEST_ATTACHMENT_MAX {
        bail!("guest attachment is too large");
    }
    Ok(())
}

fn fresh_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    data_encoding::HEXLOWER.encode(&bytes)
}

fn note_write(c: &Connection, org_id: i64, identity: &str, nonce: &str, guest: bool, source: &str) -> Result<()> {
    let cutoff = (chrono::Utc::now() - Duration::from_secs(60)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if guest {
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM org_write_log WHERE org_id = ?1 AND source_address = ?2 AND guest = 1 AND created_at >= ?3",
            params![org_id, source, cutoff],
            |r| r.get(0),
        )?;
        if n >= GUEST_WRITES_PER_MINUTE {
            bail!("guest write limit");
        }
    }
    let replay: i64 = c.query_row("SELECT COUNT(*) FROM org_write_log WHERE org_id = ?1 AND nonce = ?2", params![org_id, nonce], |r| r.get(0))?;
    if replay > 0 {
        bail!("that nonce was already used");
    }
    c.execute(
        "INSERT INTO org_write_log (org_id, identity_pub, nonce, guest, source_address, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![org_id, identity, nonce, i64::from(guest), source, now()],
    )?;
    Ok(())
}

fn public_ungated(org: &Org) -> Result<()> {
    if org.visibility != Visibility::Public || org.gated {
        bail!("not a member");
    }
    Ok(())
}

/// A member with the permission, or a guest on a public org that is not gated.
fn authorize_write(c: &Connection, org: &Org, identity: &str, permission: &str) -> Result<bool> {
    match member_of(c, org.id, identity)? {
        Some(member) => {
            if member.status != MemberStatus::Active {
                bail!("membership is still pending");
            }
            let role = role_named(c, org.id, &member.role)?;
            if !allows(&role, permission) {
                bail!("missing permission {permission}");
            }
            Ok(false)
        }
        None => {
            if org.visibility != Visibility::Public || org.gated {
                bail!("not a member");
            }
            Ok(true)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn signed_context(
    c: &Connection,
    slug: &str,
    identity: &str,
    nonce: &str,
    message: &[u8],
    signature: &[u8],
    permission: &str,
    source: &str,
    body: Option<&str>,
    attachment_len: Option<u64>,
) -> Result<(Org, String, bool)> {
    check_nonce(nonce)?;
    let source = check_source(source)?;
    let identity = check_pub(identity)?;
    verify(&identity, message, signature)?;
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let guest = authorize_write(c, &org, &identity, permission)?;
    if guest {
        if let Some(body) = body {
            check_guest_body(body)?;
        }
        if let Some(byte_len) = attachment_len {
            check_guest_attachment(byte_len)?;
        }
    }
    note_write(c, org.id, &identity, nonce, guest, &source)?;
    Ok((org, identity, guest))
}

/// Store a post with no identity. The author is [`ANONYMOUS_AUTHOR`].
///
/// The org must be public and not gated. The write counts against `source`.
pub fn submit_guest_post(c: &Connection, slug: &str, source: &str, title: &str, body: &str, visibility: Option<Visibility>) -> Result<OrgPost> {
    let source = check_source(source)?;
    check_guest_body(body)?;
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    public_ungated(&org)?;
    note_write(c, org.id, ANONYMOUS_AUTHOR, &fresh_nonce(), true, &source)?;
    insert_post(c, &org, ANONYMOUS_AUTHOR, None, title, body, visibility)
}

/// Store a reply with no identity.
#[allow(clippy::too_many_arguments)]
pub fn submit_guest_reply(c: &Connection, slug: &str, source: &str, parent_id: &str, title: &str, body: &str, visibility: Option<Visibility>) -> Result<OrgPost> {
    let source = check_source(source)?;
    check_guest_body(body)?;
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    public_ungated(&org)?;
    let parent = get_post(c, parent_id)?.ok_or_else(|| anyhow!("no post {parent_id}"))?;
    if parent.org_id != org.id || parent.archived {
        bail!("no post {parent_id}");
    }
    note_write(c, org.id, ANONYMOUS_AUTHOR, &fresh_nonce(), true, &source)?;
    insert_post(c, &org, ANONYMOUS_AUTHOR, Some(parent_id), title, body, visibility)
}

/// Record an attachment with no identity. `byte_len` is the file size the client claims.
#[allow(clippy::too_many_arguments)]
pub fn submit_guest_attachment(c: &Connection, slug: &str, source: &str, post_id: &str, name: &str, byte_len: u64, visibility: Option<Visibility>) -> Result<Asset> {
    let source = check_source(source)?;
    check_guest_attachment(byte_len)?;
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    public_ungated(&org)?;
    let post = get_post(c, post_id)?.ok_or_else(|| anyhow!("no post {post_id}"))?;
    if post.org_id != org.id || post.archived {
        bail!("no post {post_id}");
    }
    note_write(c, org.id, ANONYMOUS_AUTHOR, &fresh_nonce(), true, &source)?;
    store_attachment(c, &org, ANONYMOUS_AUTHOR, post_id, name, visibility)
}

fn store_attachment(c: &Connection, org: &Org, identity: &str, post_id: &str, name: &str, visibility: Option<Visibility>) -> Result<Asset> {
    let post = get_post(c, post_id)?.ok_or_else(|| anyhow!("no post {post_id}"))?;
    if post.org_id != org.id || post.archived {
        bail!("no post {post_id}");
    }
    let name = check_label(name, "an asset name")?;
    let identity = check_author(identity)?;
    let visibility = visibility.unwrap_or(org.default_post_visibility);
    let mut bytes = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    let id = format!("orga-{}", data_encoding::HEXLOWER.encode(&bytes));
    c.execute(
        "INSERT INTO org_assets (id, org_id, name, author_identity, visibility, custodian_identity, archived, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?4, 0, ?6)",
        params![id, org.id, name, identity, visibility.as_str(), now()],
    )?;
    list_assets(c, org.id)?.into_iter().find(|a| a.id == id).ok_or_else(|| anyhow!("asset {id} was not stored"))
}

/// Store a signed post. A non-member is rate-limited by `source`. A member with `post` is not.
#[allow(clippy::too_many_arguments)]
pub fn submit_signed_post(
    c: &Connection,
    slug: &str,
    identity: &str,
    nonce: &str,
    title: &str,
    body: &str,
    signature: &[u8],
    visibility: Option<Visibility>,
    source: &str,
) -> Result<OrgPost> {
    let message = post_message(nonce, slug, title, body);
    let (org, identity, _) = signed_context(c, slug, identity, nonce, &message, signature, "post", source, Some(body), None)?;
    insert_post(c, &org, &identity, None, title, body, visibility)
}

/// Store a signed reply on a post that is still in this organization.
#[allow(clippy::too_many_arguments)]
pub fn submit_signed_reply(
    c: &Connection,
    slug: &str,
    identity: &str,
    nonce: &str,
    parent_id: &str,
    title: &str,
    body: &str,
    signature: &[u8],
    visibility: Option<Visibility>,
    source: &str,
) -> Result<OrgPost> {
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let parent = get_post(c, parent_id)?.ok_or_else(|| anyhow!("no post {parent_id}"))?;
    if parent.org_id != org.id || parent.archived {
        bail!("no post {parent_id}");
    }
    let message = reply_message(nonce, slug, parent_id, title, body);
    let (org, identity, _) = signed_context(c, slug, identity, nonce, &message, signature, "reply", source, Some(body), None)?;
    insert_post(c, &org, &identity, Some(parent_id), title, body, visibility)
}

/// Record an attachment on a post. `byte_len` is checked for a non-member. The file bytes are a later minor.
#[allow(clippy::too_many_arguments)]
pub fn submit_signed_attachment(
    c: &Connection,
    slug: &str,
    identity: &str,
    nonce: &str,
    post_id: &str,
    name: &str,
    signature: &[u8],
    visibility: Option<Visibility>,
    source: &str,
    byte_len: u64,
) -> Result<Asset> {
    let org = super::get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let post = get_post(c, post_id)?.ok_or_else(|| anyhow!("no post {post_id}"))?;
    if post.org_id != org.id || post.archived {
        bail!("no post {post_id}");
    }
    let message = attach_message(nonce, slug, post_id, name);
    let (org, identity, _) = signed_context(c, slug, identity, nonce, &message, signature, "attach", source, None, Some(byte_len))?;
    store_attachment(c, &org, &identity, post_id, name, visibility)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orgs::{Audience, OrgEdit, create_org, get_org, get_post, org_conn, public_surface, stored_posts, update_org};

    fn conn() -> Connection {
        let d = std::env::temp_dir().join(format!(
            "acp-org-access-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        org_conn(&d.join("db.sqlite")).unwrap()
    }

    fn person() -> (NodeSecret, String) {
        let secret = NodeSecret::generate();
        let hex = pub_hex(&secret);
        (secret, hex)
    }

    fn hold(c: &Connection, owner: &str) -> Org {
        create_org(c, "hold", "Hold", "", Visibility::Private, Visibility::Private, owner).unwrap()
    }

    #[test]
    fn a_new_org_is_gated_private_and_creates_the_three_roles() {
        let c = conn();
        let (_owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        assert!(org.gated);
        assert_eq!(org.visibility, Visibility::Private);
        assert_eq!(org.registration, Registration::Closed);
        assert_eq!(org.default_register_role, "Viewer");
        assert_eq!(org.default_invite_role, "Viewer");
        let names: Vec<_> = roles(&c, org.id).unwrap().into_iter().map(|r| (r.name, r.rank, r.builtin)).collect();
        assert_eq!(names, vec![("Owner".into(), 1000, true), ("User".into(), 500, true), ("Viewer".into(), 0, true)]);
        let people = members(&c, org.id).unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].identity_pub, owner_hex);
        assert_eq!(people[0].role, "Owner");
        assert_eq!(people[0].status, MemberStatus::Active);
        assert!(stored_posts(&c, Audience::Public).unwrap().is_empty());
    }

    #[test]
    fn registration_is_closed_by_default_and_open_skips_approval() {
        let c = conn();
        let (_owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        let (guest, guest_hex) = person();
        let pending = register_member(&c, org.id, &guest_hex).unwrap();
        assert_eq!(pending.status, MemberStatus::Pending);
        assert_eq!(pending.role, "Viewer");
        let nonce = "aa11aa11aa11aa11";
        let msg = post_message(nonce, "hold", "Nope", "pending");
        let err = submit_signed_post(&c, "hold", &guest_hex, nonce, "Nope", "pending", &guest.sign(&msg), None, "203.0.113.8").unwrap_err();
        assert!(err.to_string().contains("pending"));
        let (other, _) = person();
        let err = submit_signed_post(&c, "hold", &guest_hex, nonce, "Nope", "pending", &other.sign(&msg), None, "203.0.113.8").unwrap_err();
        assert!(err.to_string().contains("signature rejected"));

        let (_viewer, viewer_hex) = person();
        let viewer_row = register_member(&c, org.id, &viewer_hex).unwrap();
        assert_eq!(viewer_row.status, MemberStatus::Pending);
        let err = approve_registration(&c, org.id, &viewer_hex, &guest_hex).unwrap_err();
        assert!(err.to_string().contains("pending"));
        approve_registration(&c, org.id, &owner_hex, &guest_hex).unwrap();
        assert_eq!(member_of(&c, org.id, &guest_hex).unwrap().unwrap().status, MemberStatus::Active);

        set_registration(&c, org.id, Registration::Open).unwrap();
        let joined = register_member(&c, org.id, &person().1).unwrap();
        assert_eq!(joined.status, MemberStatus::Active);
        assert_eq!(joined.role, "Viewer");
    }

    #[test]
    fn the_default_role_starts_at_viewer_and_the_two_joins_differ() {
        let c = conn();
        let (_owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        assert!(set_default_role(&c, org.id, Join::Register, "Owner").is_err());
        assert!(set_default_role(&c, org.id, Join::Invite, "Owner").is_err());
        set_default_role(&c, org.id, Join::Register, "User").unwrap();
        let org = get_org(&c, "hold").unwrap().unwrap();
        assert_eq!(org.default_register_role, "User");
        assert_eq!(org.default_invite_role, "Viewer");
        set_registration(&c, org.id, Registration::Open).unwrap();
        let registered = register_member(&c, org.id, &person().1).unwrap();
        assert_eq!(registered.role, "User");
        assert_eq!(registered.status, MemberStatus::Active);
        let invited = invite_member(&c, org.id, &owner_hex, &person().1).unwrap();
        assert_eq!(invited.role, "Viewer");
        assert_eq!(invited.status, MemberStatus::Active);
        set_default_role(&c, org.id, Join::Invite, "User").unwrap();
        let invited_user = invite_member(&c, org.id, &owner_hex, &person().1).unwrap();
        assert_eq!(invited_user.role, "User");
        let org = get_org(&c, "hold").unwrap().unwrap();
        assert_eq!(org.default_register_role, "User");
    }

    #[test]
    fn hierarchy_blocks_escalation_and_the_last_owner_stays() {
        let c = conn();
        let (_owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        let (_user_key, user_hex) = person();
        set_default_role(&c, org.id, Join::Invite, "User").unwrap();
        invite_member(&c, org.id, &owner_hex, &user_hex).unwrap();
        let err = assign_role(&c, org.id, &user_hex, &user_hex, "Owner").unwrap_err();
        assert!(err.to_string().contains("missing permission") || err.to_string().contains("cannot grant"));
        assert!(create_custom_role(&c, org.id, &user_hex, "Scribe", 100, &["view", "post"]).is_err());
        assert!(delete_role(&c, org.id, &owner_hex, "Owner").unwrap_err().to_string().contains("built-in"));
        assert!(delete_role(&c, org.id, &owner_hex, "Viewer").is_err());

        let scribe = create_custom_role(&c, org.id, &owner_hex, "Scribe", 100, &["view", "post"]).unwrap();
        assert_eq!(scribe.rank, 100);
        assert!(!scribe.builtin);
        let steward = create_custom_role(&c, org.id, &owner_hex, "Steward", 700, &["view", "manage_roles", "invite"]).unwrap();
        assert!(steward.rank > 0 && steward.rank < 1000);
        assert!(create_custom_role(&c, org.id, &owner_hex, "Over", 1000, &["view"]).is_err());
        assert!(create_custom_role(&c, org.id, &owner_hex, "Under", 0, &["view"]).is_err());

        let (_steward_key, steward_hex) = person();
        invite_member(&c, org.id, &owner_hex, &steward_hex).unwrap();
        assign_role(&c, org.id, &owner_hex, &steward_hex, "Steward").unwrap();
        let err = assign_role(&c, org.id, &steward_hex, &user_hex, "Owner").unwrap_err();
        assert!(err.to_string().contains("cannot grant a role at or above your own"));
        assert!(create_custom_role(&c, org.id, &steward_hex, "High", 800, &["view"]).is_err());
        assert!(create_custom_role(&c, org.id, &steward_hex, "Archivist", 50, &["archive_any"]).unwrap_err().to_string().contains("only an Owner"));
        create_custom_role(&c, org.id, &steward_hex, "Note", 50, &["view"]).unwrap();
        assign_role(&c, org.id, &steward_hex, &user_hex, "Viewer").unwrap();

        let (_second, second_hex) = person();
        invite_member(&c, org.id, &owner_hex, &second_hex).unwrap();
        assert!(assign_role(&c, org.id, &owner_hex, &owner_hex, "Viewer").unwrap_err().to_string().contains("last owner"));
        assert!(remove_member(&c, org.id, &owner_hex, &owner_hex).unwrap_err().to_string().contains("last owner"));
        assign_role(&c, org.id, &owner_hex, &second_hex, "Owner").unwrap();
        assign_role(&c, org.id, &owner_hex, &owner_hex, "Viewer").unwrap();
        assert_eq!(owner_count(&c, org.id).unwrap(), 1);
        assert!(assign_role(&c, org.id, &second_hex, &second_hex, "User").unwrap_err().to_string().contains("last owner"));
        remove_member(&c, org.id, &second_hex, &user_hex).unwrap();
        assert!(members(&c, org.id).unwrap().iter().all(|m| m.identity_pub != user_hex));
    }

    fn open_the_gate(c: &Connection, org: &Org) {
        set_gated(c, org.id, false).unwrap();
        update_org(
            c,
            &org.slug,
            &OrgEdit {
                name: None,
                description: None,
                visibility: Some(Visibility::Public),
                default_post_visibility: Some(Visibility::Public),
            },
        )
        .unwrap();
    }

    #[test]
    fn a_public_ungated_org_accepts_an_anonymous_write_and_a_signed_one() {
        let c = conn();
        let (owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        let (guest, guest_hex) = person();
        let src = "203.0.113.8";
        let nonce = "bb22bb22bb22bb22";
        let msg = post_message(nonce, "hold", "Outside", "secret-body");
        assert!(submit_guest_post(&c, "hold", src, "Outside", "secret-body", Some(Visibility::Public)).unwrap_err().to_string().contains("not a member"));
        assert!(submit_signed_post(&c, "hold", &guest_hex, nonce, "Outside", "secret-body", &guest.sign(&msg), Some(Visibility::Public), src).unwrap_err().to_string().contains("not a member"));
        open_the_gate(&c, &org);
        set_gated(&c, org.id, true).unwrap();
        assert!(submit_guest_post(&c, "hold", src, "Outside", "secret-body", Some(Visibility::Public)).unwrap_err().to_string().contains("not a member"));
        assert!(submit_signed_post(&c, "hold", &guest_hex, nonce, "Outside", "secret-body", &guest.sign(&msg), Some(Visibility::Public), src).unwrap_err().to_string().contains("not a member"));
        set_gated(&c, org.id, false).unwrap();

        let anon = submit_guest_post(&c, "hold", src, "Unsigned", "guest-body", Some(Visibility::Public)).unwrap();
        assert_eq!(anon.author_identity, ANONYMOUS_AUTHOR);
        assert_eq!(anon.custodian_identity, ANONYMOUS_AUTHOR);
        assert!(public_surface(&c).unwrap().contains("guest-body"));
        let anon_reply = submit_guest_reply(&c, "hold", src, &anon.id, "Guest reply", "guest-reply", Some(Visibility::Public)).unwrap();
        assert_eq!(anon_reply.author_identity, ANONYMOUS_AUTHOR);
        assert_eq!(anon_reply.parent_id.as_deref(), Some(anon.id.as_str()));
        let anon_file = submit_guest_attachment(&c, "hold", src, &anon.id, "note.txt", 4, Some(Visibility::Public)).unwrap();
        assert_eq!(anon_file.author_identity, ANONYMOUS_AUTHOR);
        assert!(public_surface(&c).unwrap().contains("note.txt"));
        assert!(submit_guest_post(&c, "hold", src, "Huge", &"x".repeat(GUEST_BODY_MAX + 1), Some(Visibility::Public)).unwrap_err().to_string().contains("too large"));
        assert!(submit_guest_attachment(&c, "hold", src, &anon.id, "big.bin", GUEST_ATTACHMENT_MAX + 1, Some(Visibility::Public)).unwrap_err().to_string().contains("too large"));

        let post = submit_signed_post(&c, "hold", &guest_hex, nonce, "Outside", "secret-body", &guest.sign(&msg), Some(Visibility::Public), src).unwrap();
        assert_eq!(post.author_identity, guest_hex);
        assert_ne!(post.author_identity, ANONYMOUS_AUTHOR);
        assert!(public_surface(&c).unwrap().contains("secret-body"));
        let again = submit_signed_post(&c, "hold", &guest_hex, nonce, "Outside", "secret-body", &guest.sign(&msg), None, src).unwrap_err();
        assert!(again.to_string().contains("nonce"));
        let bad = owner.sign(b"not the message");
        let err = submit_signed_post(&c, "hold", &guest_hex, "cc33cc33cc33cc33", "Outside", "secret-body", &bad, None, src).unwrap_err();
        assert!(err.to_string().contains("signature rejected"));
        let reply_nonce = "dd44dd44dd44dd44";
        let reply_msg = reply_message(reply_nonce, "hold", &post.id, "Reply", "reply-body");
        let reply = submit_signed_reply(&c, "hold", &guest_hex, reply_nonce, &post.id, "Reply", "reply-body", &guest.sign(&reply_msg), Some(Visibility::Public), src).unwrap();
        assert_eq!(reply.author_identity, guest_hex);
        let file_nonce = "ee55ee55ee55ee55";
        let file_msg = attach_message(file_nonce, "hold", &post.id, "badge.png");
        let asset = submit_signed_attachment(&c, "hold", &guest_hex, file_nonce, &post.id, "badge.png", &guest.sign(&file_msg), Some(Visibility::Public), src, 4).unwrap();
        assert_eq!(asset.author_identity, guest_hex);

        assert!(archive_post(&c, org.id, &guest_hex, &anon.id).unwrap_err().to_string().contains("not a member"));
        assert!(remove_member(&c, org.id, &guest_hex, &owner_hex).unwrap_err().to_string().contains("not a member"));
        let record = archive_post(&c, org.id, &owner_hex, &anon.id).unwrap();
        assert_eq!(record.kind, "update");
        assert_eq!(record.author_identity, ANONYMOUS_AUTHOR);
        assert_eq!(record.custodian_identity, owner_hex);
        let stored = get_post(&c, &anon.id).unwrap().unwrap();
        assert!(stored.archived);
        assert_eq!(stored.author_identity, ANONYMOUS_AUTHOR);
        assert_eq!(stored.custodian_identity, owner_hex);
        assert!(!public_surface(&c).unwrap().contains("guest-body"));

        let (viewer, viewer_hex) = person();
        invite_member(&c, org.id, &owner_hex, &viewer_hex).unwrap();
        let view_nonce = "ff66ff66ff66ff66";
        let view_msg = post_message(view_nonce, "hold", "Denied", "nope");
        let err = submit_signed_post(&c, "hold", &viewer_hex, view_nonce, "Denied", "nope", &viewer.sign(&view_msg), None, src).unwrap_err();
        assert!(err.to_string().contains("missing permission post"));

        let burst = "203.0.113.50";
        for n in 0..GUEST_WRITES_PER_MINUTE {
            let title = format!("Burst {n}");
            submit_guest_post(&c, "hold", burst, &title, "burst", Some(Visibility::Public)).unwrap();
        }
        let err = submit_guest_post(&c, "hold", burst, "Eleventh", "burst", Some(Visibility::Public)).unwrap_err();
        assert!(err.to_string().contains("guest write limit"));
        submit_guest_post(&c, "hold", "203.0.113.51", "Other address", "burst", Some(Visibility::Public)).unwrap();
        let yard = create_org(&c, "yard", "Yard", "", Visibility::Private, Visibility::Public, &owner_hex).unwrap();
        open_the_gate(&c, &yard);
        submit_guest_post(&c, "yard", burst, "Other org", "burst", Some(Visibility::Public)).unwrap();
        let member_nonce = "1234123412341234";
        let member_msg = post_message(member_nonce, "hold", "Owner still writes", "fine");
        submit_signed_post(&c, "hold", &owner_hex, member_nonce, "Owner still writes", "fine", &owner.sign(&member_msg), Some(Visibility::Public), burst).unwrap();
    }

    #[test]
    fn an_owner_removal_archives_and_public_paths_omit_it() {
        let c = conn();
        let (_owner, owner_hex) = person();
        let org = hold(&c, &owner_hex);
        open_the_gate(&c, &org);
        let (guest, guest_hex) = person();
        let nonce = "7788778877887788";
        let msg = post_message(nonce, "hold", "Keep me", "original-prose");
        let post = submit_signed_post(&c, "hold", &guest_hex, nonce, "Keep me", "original-prose", &guest.sign(&msg), Some(Visibility::Public), "203.0.113.8").unwrap();
        let file_nonce = "8899889988998899";
        let file_msg = attach_message(file_nonce, "hold", &post.id, "map.png");
        let asset = submit_signed_attachment(&c, "hold", &guest_hex, file_nonce, &post.id, "map.png", &guest.sign(&file_msg), Some(Visibility::Public), "203.0.113.9", 4).unwrap();
        assert!(public_surface(&c).unwrap().contains("original-prose"));
        assert!(public_surface(&c).unwrap().contains("map.png"));

        let record = archive_post(&c, org.id, &owner_hex, &post.id).unwrap();
        assert_eq!(record.kind, "update");
        assert!(record.archived);
        assert_eq!(record.visibility, "private");
        assert_eq!(record.custodian_identity, owner_hex);
        assert_eq!(record.author_identity, guest_hex);
        assert_eq!(record.archived_by, owner_hex);
        assert!(!record.archived_at.is_empty());
        let stored = get_post(&c, &post.id).unwrap().unwrap();
        assert!(stored.archived);
        assert_eq!(stored.visibility, Visibility::Private);
        assert_eq!(stored.custodian_identity, owner_hex);
        assert_eq!(stored.author_identity, guest_hex);
        assert_eq!(stored.archived_by.as_deref(), Some(owner_hex.as_str()));
        let owner_rows = stored_posts(&c, Audience::Owner).unwrap();
        assert!(owner_rows.iter().any(|p| p.post_id == post.id));
        assert!(!stored_posts(&c, Audience::Public).unwrap().iter().any(|p| p.post_id == post.id || p.content.contains("original-prose") || p.title.contains("Keep me")));
        let surface = public_surface(&c).unwrap();
        assert!(!surface.contains("original-prose"));
        assert!(!surface.contains("Keep me"));
        assert!(surface.contains("map.png"));

        let asset_record = archive_asset(&c, org.id, &owner_hex, &asset.id).unwrap();
        assert_eq!(asset_record.kind, "update");
        assert_eq!(asset_record.author_identity, guest_hex);
        assert_eq!(asset_record.custodian_identity, owner_hex);
        let surface = public_surface(&c).unwrap();
        assert!(!surface.contains("map.png"));
        assert!(list_assets(&c, org.id).unwrap().iter().any(|a| a.id == asset.id && a.archived));
        let rendered = format!("{surface}\n{record:?}\n{asset_record:?}");
        assert!(!rendered.contains("delete"));
        assert!(owner_rows.iter().any(|p| p.content.contains("original-prose")));
    }

    #[test]
    fn the_identity_file_is_mode_0600_and_debug_redacts_the_seed() {
        let secret = NodeSecret::generate();
        let seed_hex = data_encoding::HEXLOWER.encode(secret.sign_seed());
        let shown = format!("{secret:?}");
        assert!(shown.contains("[redacted]"));
        assert!(!shown.contains(&seed_hex));
        let dir = std::env::temp_dir().join(format!("acp-org-key-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("org-identity.key");
        let mut local = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut local);
        store_identity(&path, &secret, &local).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let loaded = load_identity(&path, &local).unwrap();
        assert_eq!(loaded.sign_pub, secret.sign_pub);
        let mut wrong = local;
        wrong[0] ^= 0xff;
        assert!(load_identity(&path, &wrong).is_err());
    }
}
