//! Organization files. Each one starts private. The bytes are mode `0600`.
//!
//! An Owner removal archives the row. It does not delete the bytes or the row.
//! A public render names an asset only when [`super::access::asset_public`] says so.

use super::access::{self, Asset, archive_asset};
use super::{Visibility, get_org, now};
use crate::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Largest file `--org-asset-add` will copy.
pub const FILE_MAX: u64 = 32 * 1024 * 1024;

/// Copy `src` into the organization. The asset starts private.
pub fn add_file(c: &Connection, db: &Path, slug: &str, actor: &str, src: &Path) -> Result<Asset> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    access::require(c, org.id, actor, "assets")?;
    let name = src.file_name().and_then(|s| s.to_str()).ok_or_else(|| anyhow!("the file needs a name"))?;
    let name = super::check_label(name, "an asset name")?;
    let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    if bytes.len() as u64 > FILE_MAX {
        bail!("an organization file is at most 32 MiB");
    }
    let mut id_bytes = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut id_bytes);
    let id = format!("orga-{}", data_encoding::HEXLOWER.encode(&id_bytes));
    let rel = format!("orgs/{slug}/assets/{id}");
    let dest = db.parent().unwrap_or(db).join(&rel);
    access::write_private(&dest, &bytes)?;
    let mode = std::fs::metadata(&dest)?.permissions().mode() & 0o777;
    if mode != 0o600 {
        bail!("asset file mode is {mode:o}, expected 600");
    }
    let mime = mime_of(&name);
    c.execute(
        "INSERT INTO org_assets (id, org_id, name, author_identity, visibility, custodian_identity, archived, path, mime, created_at)
         VALUES (?1, ?2, ?3, ?4, 'private', ?4, 0, ?5, ?6, ?7)",
        params![id, org.id, name, actor, rel, mime, now()],
    )?;
    access::list_assets(c, org.id)?.into_iter().find(|a| a.id == id).ok_or_else(|| anyhow!("asset {id} was not stored"))
}

/// Change one asset's visibility. An archived asset stays private.
pub fn set_visibility(c: &Connection, actor: &str, asset_id: &str, visibility: Visibility) -> Result<Asset> {
    let asset = find(c, asset_id)?;
    access::require(c, asset.org_id, actor, "assets")?;
    if asset.archived {
        bail!("an archived asset stays private");
    }
    c.execute(
        "UPDATE org_assets SET visibility = ?1 WHERE id = ?2",
        params![visibility.as_str(), asset.id],
    )?;
    find(c, asset_id)
}

/// Archive an asset. The row and the file stay. The replication record is an update.
pub fn remove(c: &Connection, actor: &str, asset_id: &str) -> Result<access::ReplicationUpdate> {
    let asset = find(c, asset_id)?;
    archive_asset(c, asset.org_id, actor, asset_id)
}

/// One asset by id.
pub fn find(c: &Connection, asset_id: &str) -> Result<Asset> {
    let org_id: i64 = c
        .query_row("SELECT org_id FROM org_assets WHERE id = ?1", params![asset_id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| anyhow!("no asset {asset_id}"))?;
    access::list_assets(c, org_id)?.into_iter().find(|a| a.id == asset_id).ok_or_else(|| anyhow!("no asset {asset_id}"))
}

/// Absolute path of a stored file. Empty when this asset has no bytes.
pub fn absolute_path(db: &Path, asset: &Asset) -> Option<std::path::PathBuf> {
    if asset.path.is_empty() {
        return None;
    }
    Some(db.parent().unwrap_or(db).join(&asset.path))
}

fn mime_of(name: &str) -> String {
    match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orgs::template::{self, ThemePaint};
    use crate::orgs::Audience;
    use std::os::unix::fs::PermissionsExt;

    fn conn() -> (std::path::PathBuf, Connection) {
        let d = std::env::temp_dir().join(format!(
            "acp-asset-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        let db = d.join("db.sqlite");
        let c = crate::orgs::org_conn(&db).unwrap();
        (db, c)
    }

    fn person() -> String {
        data_encoding::HEXLOWER.encode(&acp_red_star_node_core::node::NodeSecret::generate().sign_pub)
    }

    #[test]
    fn a_new_file_is_private_mode_0600_and_public_renders_omit_it_until_it_is_public() {
        let (db, c) = conn();
        let owner = person();
        crate::orgs::create_org(&c, "hold", "Hold", "", Visibility::Private, Visibility::Private, &owner).unwrap();
        let src = db.parent().unwrap().join("banner.txt");
        std::fs::write(&src, b"hello-bytes").unwrap();
        let asset = add_file(&c, &db, "hold", &owner, &src).unwrap();
        assert_eq!(asset.visibility, Visibility::Private);
        assert_eq!(asset.name, "banner.txt");
        let stored = absolute_path(&db, &asset).unwrap();
        assert_eq!(std::fs::read(&stored).unwrap(), b"hello-bytes");
        assert_eq!(std::fs::metadata(&stored).unwrap().permissions().mode() & 0o777, 0o600);
        let css = ThemePaint::neutral().css();
        let hidden = template::render_org(&c, "hold", "Red Star", "", &css, Audience::Public).unwrap();
        assert!(!hidden.contains("banner.txt"));
        set_visibility(&c, &owner, &asset.id, Visibility::Public).unwrap();
        let still = template::render_org(&c, "hold", "Red Star", "", &css, Audience::Public).unwrap();
        assert!(!still.contains("banner.txt"), "a private organization hides a public asset");
        crate::orgs::update_org(&c, "hold", &crate::orgs::OrgEdit {
            name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: None,
        }).unwrap();
        access::set_gated(&c, get_org(&c, "hold").unwrap().unwrap().id, false).unwrap();
        let open = template::render_org(&c, "hold", "Red Star", "", &css, Audience::Public).unwrap();
        assert!(open.contains("banner.txt"));
        let record = remove(&c, &owner, &asset.id).unwrap();
        assert_eq!(record.kind, "update");
        let after = template::render_org(&c, "hold", "Red Star", "", &css, Audience::Public).unwrap();
        assert!(!after.contains("banner.txt"));
        assert!(find(&c, &asset.id).unwrap().archived);
        assert!(stored.is_file(), "an archive does not delete the bytes");
    }
}
