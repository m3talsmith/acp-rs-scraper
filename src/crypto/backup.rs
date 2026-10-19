//! `*.acpbackup` archives.
//!
//! A consistent SQLite snapshot, preferences, themes, research, attachments and
//! the pinned `acp-id` are packed as tar, compressed with zstd, and sealed with
//! XChaCha20-Poly1305. The key is HKDF-SHA256 of Argon2id(password, random salt).
//! The header is a canonical CBOR array (definite lengths, no maps) and is the
//! AEAD associated data.
//!
//! `session.json` and the node secret key are never packed, so a restored
//! machine signs in again and is not a clone of the old node. Enrollment of
//! that new node is a later stage. The header's epoch list is empty until data
//! keys exist.
//!
//! The outer key is the password, not the account data key. A password-only
//! install (§7.14.1) can restore. A new machine whose header names an `acp-id`
//! also has to present the phrase or three shares, and the derived id must match.

use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use ciborium::Value;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::master::{self, AcpId};
use super::password::{Argon2Params, Stretch};
use super::pin::Pin;
use super::progress::set_stage;
use crate::config::{
    ACP_ID_REL_PATH, ATTACH_REL_PATH, DB_REL_PATH, EXPORTS_REL_PATH, LOGS_REL_DIR,
    NODE_SECRET_REL_PATH, PREFS_REL_PATH, RESEARCH_REL_DIR, SESSION_REL_PATH,
    SHARES_PENDING_REL_PATH, SHARES_VERIFIED_REL_PATH, THEMES_REL_DIR,
};

const MAGIC: &[u8] = b"ACPBACKUP";
const FORMAT_VERSION: u64 = 1;
const SCHEMA_VERSION: u64 = 1;
const BACKUP_INFO: &[u8] = b"acp-backup-v1";
const MAX_HEADER: usize = 1_048_576;
const DAY_SECS: i64 = 24 * 60 * 60;

const SCHEDULE_SQL: &str = "CREATE TABLE IF NOT EXISTS backup_schedule (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cadence TEXT NOT NULL CHECK (cadence IN ('daily', 'weekly')),
    keep_n INTEGER NOT NULL,
    dest TEXT NOT NULL,
    last_run INTEGER NOT NULL DEFAULT 0
)";

/// What a backup file says before it is decrypted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// Archive format. This build writes and reads `1` and refuses anything newer.
    pub format: u64,
    /// Schema generation recorded in the header. Migrations still run on open.
    pub schema: u64,
    /// `CARGO_PKG_VERSION` of the build that wrote the file.
    pub app: String,
    /// Unix seconds when the archive was sealed.
    pub created_at: i64,
    /// Pinned account, or empty when the backup has no identity.
    pub acp_id: String,
    /// Key epochs whose wraps travel inside the archive. Empty until data keys exist.
    pub epochs: Vec<u64>,
    /// Argon2id parameters for this archive (and for the master check on a new machine).
    pub argon2: Argon2Params,
    salt: [u8; 16],
    nonce: [u8; 24],
    content_hash: [u8; 32],
}

/// A recorded `--backup-schedule`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// `daily` or `weekly`.
    pub cadence: String,
    /// How many `*.acpbackup` files to keep in `dest`.
    pub keep: u32,
    /// Directory the scheduled backups are written to.
    pub dest: String,
    /// Unix seconds of the last successful backup, or 0 when none has run.
    pub last_run: i64,
}

/// Read the header. Does not try the password.
pub fn inspect(path: &Path) -> Result<Header> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (_ct, header) = split_file(&bytes)?;
    Ok(header)
}

/// Write `dest` from `home`. `params` are the Argon2id settings stored in the header.
pub fn write_backup(home: &Path, dest: &Path, password: &str, params: Argon2Params) -> Result<()> {
    write_backup_version(home, dest, password, params, FORMAT_VERSION)
}

/// Write `dest` using a stretch already computed for this operation.
pub(crate) fn write_backup_with(home: &Path, dest: &Path, stretch: &Stretch) -> Result<()> {
    write_backup_with_version(home, dest, stretch, FORMAT_VERSION)
}

fn write_backup_version(
    home: &Path,
    dest: &Path,
    password: &str,
    params: Argon2Params,
    format: u64,
) -> Result<()> {
    let stretch = Stretch::fresh(password, params)?;
    write_backup_with_version(home, dest, &stretch, format)
}

fn write_backup_with_version(
    home: &Path,
    dest: &Path,
    stretch: &Stretch,
    format: u64,
) -> Result<()> {
    set_stage("Snapshotting the database");
    let tar_bytes = pack(home)?;
    let content_hash = *blake3::hash(&tar_bytes).as_bytes();
    set_stage("Compressing");
    let compressed = zstd::bulk::compress(&tar_bytes, 3).context("zstd")?;
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let acp_id = Pin::load(home)?
        .map(|p| p.acp_id.to_string())
        .unwrap_or_default();
    let header = Header {
        format,
        schema: SCHEMA_VERSION,
        app: env!("CARGO_PKG_VERSION").to_string(),
        created_at: now_secs(),
        acp_id,
        epochs: epoch_list(home)?,
        argon2: stretch.params(),
        salt: *stretch.salt(),
        nonce,
        content_hash,
    };
    let header_bytes = encode_header(&header)?;
    let mut aad = Vec::with_capacity(MAGIC.len() + 4 + header_bytes.len());
    aad.extend_from_slice(MAGIC);
    aad.extend_from_slice(&(header_bytes.len() as u32).to_be_bytes());
    aad.extend_from_slice(&header_bytes);
    set_stage("Sealing the backup");
    let cipher = seal_with(stretch, &header, &aad, &compressed)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    out.write_all(&aad)?;
    out.write_all(&cipher)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Restore `src` into `home`.
///
/// `material` is the 24-word phrase or at least three shares. A new machine
/// needs it when the header names an `acp-id`. The same machine (a matching
/// pin) does not. Nothing is written if the password, the phrase or the file fails.
pub fn restore_backup(
    home: &Path,
    src: &Path,
    password: &str,
    material: Option<&str>,
) -> Result<()> {
    set_stage("Checking the backup");
    let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let (cipher, header) = split_file(&bytes)?;
    if header.format > FORMAT_VERSION {
        bail!(
            "this backup uses format {}, and this build reads up to {FORMAT_VERSION}",
            header.format
        );
    }
    if header.format == 0 {
        bail!("this backup has an unknown format");
    }
    let needs = needs_phrase(home, &header)?;
    if needs && material.is_none() {
        bail!(
            "this backup belongs to {}. A new machine needs the recovery phrase or 3 shares, plus the password.",
            header.acp_id
        );
    }
    if let Some(text) = material.filter(|_| !header.acp_id.is_empty()) {
        check_material(&header, password, text)?;
    }
    let aad_len = MAGIC.len() + 4 + header_len(&bytes)?;
    let stretch = Stretch::from_salt(password, header.salt, header.argon2)?;
    let plain = open_seal_with(&stretch, &header, &bytes[..aad_len], cipher)?;
    let tar_bytes = zstd::bulk::decompress(&plain, 512 * 1024 * 1024).context("zstd")?;
    let got = *blake3::hash(&tar_bytes).as_bytes();
    if got != header.content_hash {
        bail!("backup contents do not match the header checksum");
    }
    set_stage("Restoring files");
    unpack(home, &tar_bytes)?;
    let db = home.join(DB_REL_PATH);
    if db.is_file() && crate::crypto::atrest::is_plaintext_db(&db) {
        crate::crypto::atrest::encrypt_with_stretch(&db, &stretch)?;
    }
    Ok(())
}

/// True when restore must ask for the phrase or three shares.
/// A pin for a different account is an error, not a prompt.
pub fn needs_phrase(home: &Path, header: &Header) -> Result<bool> {
    if header.acp_id.is_empty() {
        return Ok(false);
    }
    match Pin::load(home)? {
        Some(pin) if pin.acp_id.as_str() == header.acp_id => Ok(false),
        Some(pin) => bail!(
            "this backup is for {}\nthis machine is pinned to {}\nNothing was written.",
            header.acp_id,
            pin.acp_id
        ),
        None => Ok(true),
    }
}

/// Record the schedule in the local database.
pub fn save_schedule(home: &Path, cadence: &str, keep: u32, dest: &Path) -> Result<()> {
    if cadence != "daily" && cadence != "weekly" {
        bail!("backup schedule is daily or weekly");
    }
    if keep == 0 {
        bail!("--keep must be at least 1");
    }
    let c = open_db(home)?;
    c.execute_batch(SCHEDULE_SQL)?;
    c.execute(
        "INSERT INTO backup_schedule (id, cadence, keep_n, dest, last_run) VALUES (1, ?1, ?2, ?3, 0)
         ON CONFLICT(id) DO UPDATE SET cadence = ?1, keep_n = ?2, dest = ?3",
        rusqlite::params![cadence, keep, dest.display().to_string()],
    )?;
    Ok(())
}

/// The schedule, if one was saved and the database is readable.
pub fn load_schedule(home: &Path) -> Result<Option<Schedule>> {
    let path = home.join(DB_REL_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let c = open_db(home)?;
    let present: Option<i64> = c
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'backup_schedule'",
            [],
            |r| r.get(0),
        )
        .ok();
    if present.is_none() {
        return Ok(None);
    }
    let row = c
        .query_row(
            "SELECT cadence, keep_n, dest, last_run FROM backup_schedule WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    Ok(row.map(|(cadence, keep, dest, last_run)| Schedule {
        cadence,
        keep,
        dest,
        last_run,
    }))
}

/// Remember that a backup just succeeded.
pub fn mark_backup_ran(home: &Path) -> Result<()> {
    if load_schedule(home)?.is_none() {
        return Ok(());
    }
    let c = open_db(home)?;
    c.execute(
        "UPDATE backup_schedule SET last_run = ?1 WHERE id = 1",
        rusqlite::params![now_secs()],
    )?;
    Ok(())
}

/// Whether `schedule` is due at `now` (unix seconds).
pub fn schedule_due(schedule: &Schedule, now: i64) -> bool {
    let window = if schedule.cadence == "weekly" {
        7 * DAY_SECS
    } else {
        DAY_SECS
    };
    schedule.last_run == 0 || now.saturating_sub(schedule.last_run) >= window
}

/// Status line when a schedule is due. `None` when there is no schedule or it is current.
pub fn schedule_reminder(home: &Path) -> Option<String> {
    let schedule = load_schedule(home).ok()??;
    if schedule_due(&schedule, now_secs()) {
        Some(format!(
            "A {} backup is due. Run --backup. Destination: {}",
            schedule.cadence, schedule.dest
        ))
    } else {
        None
    }
}

/// Delete the oldest `*.acpbackup` files in `dir` until `keep` remain.
pub fn prune_backups(dir: &Path, keep: u32) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("acpbackup") {
            continue;
        }
        let modified = entry.metadata()?.modified().unwrap_or(UNIX_EPOCH);
        files.push((modified, path));
    }
    files.sort_by_key(|(m, _)| *m);
    let keep = keep as usize;
    if files.len() <= keep {
        return Ok(0);
    }
    let extra = files.len() - keep;
    for (_, path) in files.into_iter().take(extra) {
        std::fs::remove_file(path)?;
    }
    Ok(extra)
}

/// Default path under the exports directory.
pub fn default_backup_path(home: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    home.join(EXPORTS_REL_PATH)
        .join(format!("backup-{stamp}.acpbackup"))
}

fn check_material(header: &Header, password: &str, text: &str) -> Result<()> {
    let seed = master::seed_from_material(text)?;
    let phrase = master::phrase_from_entropy(&seed)?;
    let master = master::derive_master(&phrase, password, header.argon2)?;
    let expected = AcpId::parse(&header.acp_id)?;
    master.require_id(&expected)?;
    Ok(())
}

fn seal_with(stretch: &Stretch, header: &Header, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let key = stretch.key(BACKUP_INFO);
    let cipher = XChaCha20Poly1305::new((&*key).into());
    cipher
        .encrypt((&header.nonce).into(), Payload { msg: plain, aad })
        .map_err(|_| anyhow::anyhow!("backup encryption failed"))
}

fn open_seal_with(
    stretch: &Stretch,
    header: &Header,
    aad: &[u8],
    cipher_text: &[u8],
) -> Result<Vec<u8>> {
    let key = stretch.key(BACKUP_INFO);
    let cipher = XChaCha20Poly1305::new((&*key).into());
    cipher
        .decrypt(
            (&header.nonce).into(),
            Payload { msg: cipher_text, aad },
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "the password does not open this backup, or the file is damaged. Nothing was written."
            )
        })
}

fn split_file(bytes: &[u8]) -> Result<(&[u8], Header)> {
    if bytes.len() < MAGIC.len() + 4 || &bytes[..MAGIC.len()] != MAGIC {
        bail!("not an acpbackup file");
    }
    let n = header_len(bytes)?;
    let start = MAGIC.len() + 4;
    let end = start + n;
    if bytes.len() < end + 16 {
        bail!("truncated backup");
    }
    let header = decode_header(&bytes[start..end])?;
    Ok((&bytes[end..], header))
}

fn header_len(bytes: &[u8]) -> Result<usize> {
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&bytes[MAGIC.len()..MAGIC.len() + 4]);
    let n = u32::from_be_bytes(len_bytes) as usize;
    if n == 0 || n > MAX_HEADER {
        bail!("backup header length is not usable");
    }
    Ok(n)
}

fn encode_header(header: &Header) -> Result<Vec<u8>> {
    let epochs = Value::Array(
        header
            .epochs
            .iter()
            .copied()
            .map(|e| Value::Integer(e.into()))
            .collect(),
    );
    let value = Value::Array(vec![
        Value::Integer(header.format.into()),
        Value::Integer(header.schema.into()),
        Value::Text(header.app.clone()),
        Value::Integer(header.created_at.into()),
        Value::Text(header.acp_id.clone()),
        epochs,
        Value::Integer(u64::from(header.argon2.m_kib).into()),
        Value::Integer(u64::from(header.argon2.t).into()),
        Value::Integer(u64::from(header.argon2.p).into()),
        Value::Bytes(header.salt.to_vec()),
        Value::Bytes(header.nonce.to_vec()),
        Value::Bytes(header.content_hash.to_vec()),
    ]);
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&value, &mut buf).context("encoding backup header")?;
    Ok(buf)
}

fn decode_header(bytes: &[u8]) -> Result<Header> {
    let value: Value = ciborium::de::from_reader(bytes).context("backup header")?;
    let Value::Array(a) = value else {
        bail!("backup header is not an array");
    };
    if a.len() != 12 {
        bail!("backup header has {} fields", a.len());
    }
    let salt = as_bytes(&a[9], 16)?;
    let nonce = as_bytes(&a[10], 24)?;
    let hash = as_bytes(&a[11], 32)?;
    let mut salt_a = [0u8; 16];
    let mut nonce_a = [0u8; 24];
    let mut hash_a = [0u8; 32];
    salt_a.copy_from_slice(&salt);
    nonce_a.copy_from_slice(&nonce);
    hash_a.copy_from_slice(&hash);
    let epochs = match &a[5] {
        Value::Array(items) => items.iter().map(as_u64).collect::<Result<Vec<_>>>()?,
        _ => bail!("backup epochs are not a list"),
    };
    Ok(Header {
        format: as_u64(&a[0])?,
        schema: as_u64(&a[1])?,
        app: as_text(&a[2])?,
        created_at: as_u64(&a[3])? as i64,
        acp_id: as_text(&a[4])?,
        epochs,
        argon2: Argon2Params {
            m_kib: as_u64(&a[6])? as u32,
            t: as_u64(&a[7])? as u32,
            p: as_u64(&a[8])? as u32,
        },
        salt: salt_a,
        nonce: nonce_a,
        content_hash: hash_a,
    })
}

fn as_u64(v: &Value) -> Result<u64> {
    match v {
        Value::Integer(i) => i64::try_from(*i)
            .ok()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| anyhow::anyhow!("backup header integer is out of range")),
        _ => bail!("backup header field is not an integer"),
    }
}

fn as_text(v: &Value) -> Result<String> {
    match v {
        Value::Text(s) => Ok(s.clone()),
        _ => bail!("backup header field is not text"),
    }
}

fn as_bytes(v: &Value, len: usize) -> Result<Vec<u8>> {
    match v {
        Value::Bytes(b) if b.len() == len => Ok(b.clone()),
        Value::Bytes(_) => bail!("backup header field has the wrong length"),
        _ => bail!("backup header field is not bytes"),
    }
}

fn pack(home: &Path) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut body);
        if home.join(DB_REL_PATH).is_file() {
            let snap = snapshot_db(home)?;
            append_file(&mut builder, DB_REL_PATH, &snap)?;
        }
        for rel in [
            PREFS_REL_PATH,
            ACP_ID_REL_PATH,
            SHARES_PENDING_REL_PATH,
            SHARES_VERIFIED_REL_PATH,
        ] {
            let path = home.join(rel);
            if path.is_file() {
                append_file(&mut builder, rel, &std::fs::read(&path)?)?;
            }
        }
        for rel in [THEMES_REL_DIR, RESEARCH_REL_DIR, ATTACH_REL_PATH] {
            append_tree(&mut builder, home, Path::new(rel))?;
        }
        builder.finish()?;
    }
    Ok(body)
}

/// Epoch numbers stored in `key_epochs`, when that table exists.
fn epoch_list(home: &Path) -> Result<Vec<u64>> {
    let path = home.join(DB_REL_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let conn = crate::storage::db::open_db(&path)?;
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'key_epochs'",
        [],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(Vec::new());
    }
    let mut st = conn.prepare("SELECT epoch FROM key_epochs ORDER BY epoch")?;
    let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let n = row?;
        if n < 0 {
            bail!("a key epoch is negative");
        }
        out.push(n as u64);
    }
    Ok(out)
}

fn snapshot_db(home: &Path) -> Result<Vec<u8>> {
    let src_path = home.join(DB_REL_PATH);
    let mut nonce = [0u8; 4];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let tmp = std::env::temp_dir().join(format!(
        "acp-snap-{}-{}",
        std::process::id(),
        data_encoding::HEXLOWER.encode(&nonce)
    ));
    let src = crate::storage::db::open_db(&src_path)?;
    let path = tmp.display().to_string().replace('\'', "''");
    // The online backup API refuses an encrypted database. sqlcipher_export
    // writes a plaintext snapshot, which the archive then seals.
    src.execute_batch(&format!(
        "ATTACH DATABASE '{path}' AS plaintext KEY '';
         PRAGMA plaintext.synchronous = OFF;
         PRAGMA plaintext.journal_mode = OFF;
         SELECT sqlcipher_export('plaintext');
         DETACH DATABASE plaintext;"
    ))
    .context("snapshot")?;
    drop(src);
    let bytes = std::fs::read(&tmp).context("reading the database snapshot")?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

fn append_file(builder: &mut tar::Builder<&mut Vec<u8>>, rel: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_path(rel)?;
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder.append(&header, data)?;
    Ok(())
}

fn append_tree(builder: &mut tar::Builder<&mut Vec<u8>>, home: &Path, rel: &Path) -> Result<()> {
    let dir = home.join(rel);
    if !dir.is_dir() {
        return Ok(());
    }
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_symlink() {
                continue;
            }
            let rel_path = path.strip_prefix(home).unwrap_or(path.as_path());
            if skip_source(rel_path) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.is_file() {
                let data = std::fs::read(&path)?;
                let name = tar_name(rel_path);
                append_file(builder, &name, &data)?;
            }
        }
    }
    Ok(())
}

/// Files the packer must not read from the live home. The database is added
/// from the snapshot instead, so the live file is skipped here.
fn skip_source(rel: &Path) -> bool {
    let name = tar_name(rel);
    name == DB_REL_PATH || never_restore(&name)
}

/// Paths a restore must not write, even if an older archive contained them.
fn never_restore(name: &str) -> bool {
    name == SESSION_REL_PATH
        || name == NODE_SECRET_REL_PATH
        || name.starts_with(LOGS_REL_DIR)
        || name.starts_with(".acp/cache")
        || name.starts_with(EXPORTS_REL_PATH)
        || name.ends_with(".acpbackup")
        || name.ends_with(".bootstrap")
        || name.ends_with(".prev")
        || name.ends_with(".encrypting")
}

fn tar_name(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn unpack(home: &Path, tar_bytes: &[u8]) -> Result<()> {
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            bail!("backup path escapes the home directory");
        }
        let rel = tar_name(&path);
        if rel.is_empty() || never_restore(&rel) {
            continue;
        }
        let dest = home.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        let mut file = File::create(&dest)?;
        file.write_all(&data)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

fn open_db(home: &Path) -> Result<rusqlite::Connection> {
    crate::storage::db::open_db(&home.join(DB_REL_PATH))
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
fn write_newer(home: &Path, dest: &Path, password: &str) -> Result<()> {
    write_backup_version(home, dest, password, Argon2Params::FAST, 99)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct horse battery staple";
    const SENTINEL: &str = "post-sentinel-unique-string";

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "acp-bak-{}-{}-{}",
            label,
            std::process::id(),
            now_secs()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_home(home: &Path) {
        let db = home.join(DB_REL_PATH);
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE posts (id INTEGER PRIMARY KEY, content TEXT);
             INSERT INTO posts (content) VALUES ('{SENTINEL}');"
        ))
        .unwrap();
        drop(c);
        std::fs::write(home.join(PREFS_REL_PATH), "email = \"a@b.c\"\n").unwrap();
        std::fs::create_dir_all(home.join(THEMES_REL_DIR)).unwrap();
        std::fs::write(
            home.join(THEMES_REL_DIR).join("mine.toml"),
            "name = \"mine\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(home.join(RESEARCH_REL_DIR).join("strike")).unwrap();
        std::fs::write(
            home.join(RESEARCH_REL_DIR).join("strike").join("note.txt"),
            "notes",
        )
        .unwrap();
        std::fs::write(home.join(SESSION_REL_PATH), "SESSION-SENTINEL-do-not-copy").unwrap();
        let node = home.join(NODE_SECRET_REL_PATH);
        std::fs::create_dir_all(node.parent().unwrap()).unwrap();
        std::fs::write(&node, "NODE-SECRET-SENTINEL").unwrap();
    }

    fn post_content(home: &Path) -> String {
        let db = home.join(DB_REL_PATH);
        crate::crypto::atrest::unlock(&db, PASSWORD).unwrap();
        let c = crate::storage::db::open_db(&db).unwrap();
        c.query_row("SELECT content FROM posts", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn fresh_install_restore_round_trips_and_skips_session_and_node_key() {
        let src = scratch("src");
        seed_home(&src);
        let file = src.join("out.acpbackup");
        write_backup(&src, &file, PASSWORD, Argon2Params::FAST).unwrap();
        let raw = std::fs::read(&file).unwrap();
        assert!(
            !raw.windows(SENTINEL.len())
                .any(|w| w == SENTINEL.as_bytes())
        );
        assert!(!raw.windows(24).any(|w| w == b"SESSION-SENTINEL-do-not"));
        assert!(!raw.windows(18).any(|w| w == b"NODE-SECRET-SENTINEL"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let fresh = scratch("fresh");
        restore_backup(&fresh, &file, PASSWORD, None).unwrap();
        assert_eq!(post_content(&fresh), SENTINEL);
        assert_eq!(
            std::fs::read_to_string(fresh.join(PREFS_REL_PATH)).unwrap(),
            "email = \"a@b.c\"\n"
        );
        assert!(fresh.join(THEMES_REL_DIR).join("mine.toml").is_file());
        assert!(
            fresh
                .join(RESEARCH_REL_DIR)
                .join("strike")
                .join("note.txt")
                .is_file()
        );
        assert!(!fresh.join(SESSION_REL_PATH).exists());
        assert!(!fresh.join(NODE_SECRET_REL_PATH).exists());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    #[test]
    fn wrong_password_tamper_truncate_and_newer_format_are_refused() {
        let home = scratch("bad");
        seed_home(&home);
        let file = home.join("out.acpbackup");
        write_backup(&home, &file, PASSWORD, Argon2Params::FAST).unwrap();
        let dest = scratch("bad-dest");
        let err = restore_backup(&dest, &file, "correct horse battery staple!", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("password") || err.contains("damaged"), "{err}");
        assert!(!err.contains("correct horse battery staple!"), "{err}");
        assert!(!dest.join(DB_REL_PATH).exists());

        let mut flipped = std::fs::read(&file).unwrap();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x5a;
        let bad = home.join("flipped.acpbackup");
        std::fs::write(&bad, &flipped).unwrap();
        assert!(restore_backup(&dest, &bad, PASSWORD, None).is_err());
        assert!(!dest.join(DB_REL_PATH).exists());

        std::fs::write(home.join("short.acpbackup"), &flipped[..20]).unwrap();
        assert!(restore_backup(&dest, &home.join("short.acpbackup"), PASSWORD, None).is_err());

        let newer = home.join("newer.acpbackup");
        write_newer(&home, &newer, PASSWORD).unwrap();
        let err = restore_backup(&dest, &newer, PASSWORD, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("format"), "{err}");
        assert!(!dest.join(DB_REL_PATH).exists());
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn a_new_machine_needs_the_phrase_and_a_wrong_phrase_is_refused() {
        let entropy = [0x42u8; 32];
        let phrase = master::phrase_from_entropy(&entropy).unwrap();
        let master = master::derive_master(&phrase, PASSWORD, Argon2Params::FAST).unwrap();
        let src = scratch("id-src");
        seed_home(&src);
        Pin {
            acp_id: master.acp_id.clone(),
            argon2: Argon2Params::FAST,
            sign_pub: master.sign_pub,
            wrap_pub: master.wrap_pub,
        }
        .store(&src)
        .unwrap();
        let file = src.join("out.acpbackup");
        write_backup(&src, &file, PASSWORD, Argon2Params::FAST).unwrap();
        let header = inspect(&file).unwrap();
        assert_eq!(header.acp_id, master.acp_id.as_str());
        assert!(header.epochs.is_empty());

        let fresh = scratch("id-fresh");
        assert!(needs_phrase(&fresh, &header).unwrap());
        let err = restore_backup(&fresh, &file, PASSWORD, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("phrase") || err.contains("shares"), "{err}");
        assert!(!fresh.join(DB_REL_PATH).exists());

        let wrong = master::phrase_from_entropy(&[0x43u8; 32]).unwrap();
        let err = restore_backup(&fresh, &file, PASSWORD, Some(&wrong))
            .unwrap_err()
            .to_string();
        assert!(err.contains(master.acp_id.as_str()), "{err}");
        assert!(!err.contains(&phrase), "{err}");
        assert!(!err.contains(PASSWORD), "{err}");
        assert!(!fresh.join(DB_REL_PATH).exists());

        restore_backup(&fresh, &file, PASSWORD, Some(&phrase)).unwrap();
        assert_eq!(post_content(&fresh), SENTINEL);
        assert_eq!(
            Pin::load(&fresh).unwrap().unwrap().acp_id.as_str(),
            master.acp_id.as_str()
        );

        let shares = super::super::slip39::split_seed(&entropy).unwrap();
        let other = scratch("id-shares");
        let text = shares[..3].join("\n");
        restore_backup(&other, &file, PASSWORD, Some(&text)).unwrap();
        assert_eq!(post_content(&other), SENTINEL);

        let same = scratch("id-same");
        Pin {
            acp_id: master.acp_id.clone(),
            argon2: Argon2Params::FAST,
            sign_pub: master.sign_pub,
            wrap_pub: master.wrap_pub,
        }
        .store(&same)
        .unwrap();
        assert!(!needs_phrase(&same, &header).unwrap());
        std::fs::create_dir_all(same.join(DB_REL_PATH).parent().unwrap()).unwrap();
        std::fs::write(same.join(DB_REL_PATH), b"not a database").unwrap();
        restore_backup(&same, &file, PASSWORD, None).unwrap();
        assert_eq!(post_content(&same), SENTINEL);

        let other_id = master::derive_master(
            &master::phrase_from_entropy(&[0x44u8; 32]).unwrap(),
            PASSWORD,
            Argon2Params::FAST,
        )
        .unwrap();
        let wrong_machine = scratch("id-other");
        Pin {
            acp_id: other_id.acp_id.clone(),
            argon2: Argon2Params::FAST,
            sign_pub: other_id.sign_pub,
            wrap_pub: other_id.wrap_pub,
        }
        .store(&wrong_machine)
        .unwrap();
        assert!(needs_phrase(&wrong_machine, &header).is_err());
        assert!(restore_backup(&wrong_machine, &file, PASSWORD, None).is_err());
        assert!(!wrong_machine.join(DB_REL_PATH).exists());

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&fresh);
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&same);
        let _ = std::fs::remove_dir_all(&wrong_machine);
    }

    #[test]
    fn schedule_is_due_until_a_backup_runs_and_old_files_are_pruned() {
        let home = scratch("sched");
        assert!(load_schedule(&home).unwrap().is_none());
        let dest = home.join("backups");
        std::fs::create_dir_all(&dest).unwrap();
        save_schedule(&home, "daily", 2, &dest).unwrap();
        let loaded = load_schedule(&home).unwrap().unwrap();
        assert_eq!(loaded.cadence, "daily");
        assert!(schedule_due(&loaded, now_secs()));
        assert!(schedule_due(
            &Schedule {
                last_run: 1_000,
                ..loaded.clone()
            },
            1_000 + DAY_SECS
        ));
        assert!(!schedule_due(
            &Schedule {
                last_run: 1_000,
                cadence: "weekly".into(),
                ..loaded.clone()
            },
            1_000 + DAY_SECS
        ));
        assert!(schedule_reminder(&home).unwrap().contains("--backup"));
        mark_backup_ran(&home).unwrap();
        assert!(schedule_reminder(&home).is_none());
        for name in ["a.acpbackup", "b.acpbackup", "c.acpbackup", "skip.txt"] {
            std::fs::write(dest.join(name), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(prune_backups(&dest, 2).unwrap(), 1);
        assert!(!dest.join("a.acpbackup").exists());
        assert!(dest.join("c.acpbackup").exists());
        assert!(dest.join("skip.txt").exists());
        let _ = std::fs::remove_dir_all(&home);
    }
}
