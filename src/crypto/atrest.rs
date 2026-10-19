//! Encryption at rest.
//!
//! The on-disk database is SQLCipher. Its key is a random 256-bit local key, passed
//! with `PRAGMA key = "x'<hex>'"` so SQLCipher does not run its 256,000-iteration
//! password KDF. SQLCipher still derives the page HMAC key with 2 iterations of
//! that KDF; that step is part of the raw-key construction and is not a second
//! password stretch. The local key is wrapped with XChaCha20-Poly1305 under
//! HKDF(Argon2id(password)) and stored in a bootstrap file next to the database.
//! The bootstrap cannot live inside the database: the database cannot be opened
//! without it.
//!
//! One operation (creating a database, migrating a plaintext file, writing a
//! backup, restoring) stretches the password once. The local key, the account
//! key, and the backup key are HKDF labels of that stretch. A password change
//! stretches the old password and the new password, once each, when the wraps
//! share a salt. A recovery-phrase seal has its own salt, so opening it is one
//! more stretch. The new seal reuses the new password's stretch.
//!
//! A separate account data key, one per epoch, is what a lost node would rotate.
//! It is wrapped to the local password and, when an `acp-id` is pinned, to the
//! master wrapping key with HPKE (X25519, HKDF-SHA256, ChaCha20-Poly1305). A node
//! recipient is accepted by the same decoder and is not produced here.
//!
//! The program refuses to start on a 2.x plaintext database and tells you to run
//! `--encrypt-db`. A database this program creates is encrypted. Library callers
//! that never ask for a password, including the unit tests, still get a plaintext file.

use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use ciborium::Value;
#[cfg(test)]
use hpke::kem::X25519HkdfSha256;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use zeroize::Zeroizing;

use super::master::Master;
use super::password::{Argon2Params, Stretch};
use super::pin::Pin;
use super::progress::set_stage;
use crate::storage::schema::{
    BACKUPS_SCHEMA, IDENTITY_CERTS_SCHEMA, KEY_EPOCHS_SCHEMA, QUARANTINE_SCHEMA, REVOCATIONS_SCHEMA,
};

const BOOT_VERSION: u64 = 1;
const LOCAL_INFO: &[u8] = b"acp-local-key-v1";
const ACCOUNT_PASS_INFO: &[u8] = b"acp-account-pass-v1";
const KIND_PASS: u64 = 0;
const KIND_MASTER: u64 = 1;
const KIND_NODE: u64 = 2;

#[cfg(test)]
type HpkeKem = X25519HkdfSha256;

fn cache() -> &'static Mutex<HashMap<PathBuf, Zeroizing<[u8; 32]>>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<PathBuf, Zeroizing<[u8; 32]>>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(db: &Path) -> PathBuf {
    std::fs::canonicalize(db).unwrap_or_else(|_| db.to_path_buf())
}

/// Remember `key` as the local key for `db` for the rest of this process.
///
/// Job children receive the key on stdin and call this. The bytes are copied
/// into a [`Zeroizing`] buffer; the caller's copy is theirs to wipe.
pub(crate) fn install_cached_key(db: &Path, key: &[u8; 32]) {
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), Zeroizing::new(*key));
}

/// The local key for `db`, when this process has already unlocked it.
pub(crate) fn cached_key(db: &Path) -> Option<Zeroizing<[u8; 32]>> {
    cache().lock().expect("key cache").get(&cache_key(db)).cloned()
}

/// Whether this process can open `db` without a password.
pub(crate) fn is_unlocked(db: &Path) -> bool {
    cached_key(db).is_some()
}

/// Drop the cached key for `db`. Other databases in this process stay unlocked.
#[cfg(test)]
pub(crate) fn forget(db: &Path) {
    cache().lock().expect("key cache").remove(&cache_key(db));
}

/// Drop every cached database key. Each value is zeroed as it is removed.
pub(crate) fn clear_cached_keys() {
    cache().lock().expect("key cache").clear();
}

/// Wipes every cached database key when the process command returns.
///
/// The cache is a process-lifetime map, so leaving the process does not by
/// itself run [`Zeroizing`]'s drop. Hold one of these for the whole command.
pub(crate) struct KeyWipe;

impl Drop for KeyWipe {
    fn drop(&mut self) {
        clear_cached_keys();
    }
}

/// If `ACP_INHERIT_DB_KEY=1` and stdin is not a terminal, read the 32-byte local
/// key from stdin and cache it.
///
/// The variable is only a signal. The key is the first 32 bytes of stdin, never
/// the environment and never argv. A terminal is ignored, so an exported flag
/// cannot take keystrokes as a key. The flag is left in this process (it is not
/// a secret). A job that does not receive a key clears it for that child.
pub(crate) fn accept_inherited_key() -> Result<()> {
    let armed = std::env::var_os("ACP_INHERIT_DB_KEY").is_some_and(|v| v == "1");
    if !armed || std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(());
    }
    let mut buf = Zeroizing::new([0u8; 32]);
    std::io::stdin()
        .read_exact(buf.as_mut())
        .context("the unlocked database key was not passed on stdin")?;
    let db = crate::config::home()?.join(crate::config::DB_REL_PATH);
    install_cached_key(&db, &buf);
    Ok(())
}

/// Bootstrap file for `db` (`rs.database.bootstrap`).
pub fn bootstrap_path(db: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bootstrap", db.display()))
}

/// Whether `path` is a plaintext SQLite file.
pub fn is_plaintext_db(path: &Path) -> bool {
    header(path).is_some_and(|b| &b == b"SQLite format 3\0")
}

/// What to print when `db` is a 2.x plaintext database the program must not open.
///
/// `None` when the file is missing, already encrypted, or not a SQLite file.
pub fn legacy_plaintext_notice(db: &Path) -> Option<String> {
    if !db.is_file() || !is_plaintext_db(db) || bootstrap_path(db).is_file() {
        return None;
    }
    Some(format!(
        "{path} is a 2.x database and is not encrypted. 3.0.0 will not open it.\n\
         \n\
         Migrate it with a backup first:\n\
         \n\
           acp-rs-scraper --encrypt-db\n\
         \n\
         That writes a *.acpbackup, encrypts the database, and removes the plaintext \
         file only after the encrypted copy matches. It asks for the password the \
         database will use from then on.\n\
         \n\
         A 2.x binary still reads this file until that command succeeds. After it \
         succeeds, 2.x cannot read the database. Copy the file somewhere outside \
         ~/.acp first if you may need to go back. If migration fails, the original \
         file is left in place.",
        path = db.display()
    ))
}

/// A file long enough to be a database and not a plaintext SQLite header.
fn looks_encrypted(path: &Path) -> bool {
    header(path).is_some_and(|b| &b != b"SQLite format 3\0")
}

fn header(path: &Path) -> Option<[u8; 16]> {
    let mut buf = [0u8; 16];
    let mut f = File::open(path).ok()?;
    (f.read(&mut buf).ok()? == 16).then_some(buf)
}

/// Apply the cached local key, or `ACP_PASSWORD` when a bootstrap file is present.
/// A plaintext database with no bootstrap is left untouched.
pub fn apply_if_needed(conn: &rusqlite::Connection, db: &Path) -> Result<()> {
    if let Some(key) = cache()
        .lock()
        .expect("key cache")
        .get(&cache_key(db))
        .cloned()
    {
        return apply_key(conn, &key);
    }
    let boot = bootstrap_path(db);
    if !boot.is_file() {
        if looks_encrypted(db) {
            bail!(
                "the database at {} is encrypted and its bootstrap file is missing",
                db.display()
            );
        }
        return Ok(());
    }
    let Some(password) = std::env::var("ACP_PASSWORD").ok().filter(|p| !p.is_empty()) else {
        bail!(
            "the database at {} is encrypted. Unlock it from a terminal, or set ACP_PASSWORD for a non-interactive run.",
            db.display()
        );
    };
    let key = unwrap_local(&boot, &password)?;
    apply_key(conn, &key)?;
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), key);
    Ok(())
}

/// Unlock an existing encrypted database and remember the local key for this process.
pub fn unlock(db: &Path, password: &str) -> Result<()> {
    let _ = unlock_stretch(db, password)?;
    Ok(())
}

/// Unlock, and return the password stretch so a backup can seal without a second Argon2id.
pub(crate) fn unlock_stretch(db: &Path, password: &str) -> Result<Stretch> {
    let (key, stretch) = open_local(&bootstrap_path(db), password)?;
    set_stage("Opening the database");
    let conn =
        rusqlite::Connection::open(db).with_context(|| format!("opening {}", db.display()))?;
    apply_key(&conn, &key)?;
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), key);
    Ok(stretch)
}

/// Create an encrypted database, or unlock one, or leave a legacy plaintext file alone.
/// A missing database is created encrypted. `password` is required for those two cases.
pub fn prepare(db: &Path, password: Option<&str>) -> Result<()> {
    if bootstrap_path(db).is_file() {
        if cache()
            .lock()
            .expect("key cache")
            .contains_key(&cache_key(db))
        {
            return Ok(());
        }
        let password = password.ok_or_else(|| {
            anyhow::anyhow!("the database is encrypted and no password was provided")
        })?;
        return unlock(db, password);
    }
    if db.is_file() {
        if is_plaintext_db(db) {
            return Ok(());
        }
        bail!(
            "the database at {} is encrypted and its bootstrap file is missing",
            db.display()
        );
    }
    let password = password
        .ok_or_else(|| anyhow::anyhow!("a new database is encrypted and needs a password"))?;
    super::password::check_password(password)?;
    create_new(db, password, chosen_params(db)?)
}

/// Convert a plaintext database. Makes a backup first, checks counts and a content
/// hash, and deletes the plaintext file only after the encrypted file opens.
pub fn encrypt_db(db: &Path, password: &str, backup_to: &Path) -> Result<()> {
    if !db.is_file() {
        bail!("there is no database to encrypt at {}", db.display());
    }
    if bootstrap_path(db).is_file() || !is_plaintext_db(db) {
        bail!("the database is already encrypted");
    }
    super::password::check_password(password)?;
    let home = parent_home(db)?;
    let params = chosen_params(db)?;
    let stretch = Stretch::fresh(password, params)?;
    super::backup::write_backup_with(home, backup_to, &stretch)
        .context("backup before encryption")?;
    encrypt_with_stretch(db, &stretch)
}

/// Replace a plaintext database at `db` with an encrypted one. No backup.
pub fn encrypt_in_place(db: &Path, password: &str, params: Argon2Params) -> Result<()> {
    let stretch = Stretch::fresh(password, params)?;
    encrypt_with_stretch(db, &stretch)
}

/// Encrypt `db` with an already computed password stretch.
pub(crate) fn encrypt_with_stretch(db: &Path, stretch: &Stretch) -> Result<()> {
    if bootstrap_path(db).is_file() || !is_plaintext_db(db) {
        bail!("the database is already encrypted");
    }
    set_stage("Checking the database");
    let before = fingerprint(db, None)?;
    let local = random32();
    let tmp = PathBuf::from(format!("{}.encrypting", db.display()));
    let _ = std::fs::remove_file(&tmp);
    set_stage("Encrypting database");
    export_encrypted(db, &tmp, &local)?;
    set_stage("Verifying");
    let after = fingerprint(&tmp, Some(&local))?;
    if before != after {
        let _ = std::fs::remove_file(&tmp);
        bail!(
            "encrypted copy does not match the plaintext database; the original was left in place"
        );
    }
    let prev = PathBuf::from(format!("{}.prev", db.display()));
    let _ = std::fs::remove_file(&prev);
    std::fs::rename(db, &prev).context("moving the plaintext database aside")?;
    if let Err(e) = std::fs::rename(&tmp, db) {
        let _ = std::fs::rename(&prev, db);
        return Err(e).context("installing the encrypted database");
    }
    set_stage("Saving the key");
    if let Err(e) = write_bootstrap(db, stretch, &local) {
        let _ = std::fs::rename(db, &tmp);
        let _ = std::fs::rename(&prev, db);
        return Err(e);
    }
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), local.clone());
    set_stage("Verifying");
    let check = fingerprint(db, Some(&local))?;
    if check != before {
        bail!(
            "the installed database does not match; {} was kept",
            prev.display()
        );
    }
    set_stage("Saving the account key");
    seal_epoch(db, stretch)?;
    let _ = std::fs::remove_file(&prev);
    Ok(())
}

/// Add a master HPKE wrap of the current account key, if a database and an epoch exist.
pub fn add_master_recipient(db: &Path, password: &str, master: &Master) -> Result<()> {
    if !bootstrap_path(db).is_file() {
        return Ok(());
    }
    let stretch = unlock_stretch(db, password)?;
    let conn = rusqlite::Connection::open(db)?;
    apply_if_needed(&conn, db)?;
    ensure_tables(&conn)?;
    let Some((epoch, wraps)) = current_epoch(&conn)? else {
        return Ok(());
    };
    let dek = unwrap_account(&wraps, password, Some(master), Some(&stretch))?;
    if wraps_to_master(&wraps, &master.wrap_pub) {
        return Ok(());
    }
    let mut next = wraps;
    next.push(seal_master(&master.wrap_pub, &dek)?);
    store_wraps(&conn, epoch, &next)?;
    Ok(())
}

/// Rewrap the local key and the account passphrase wrap with a new password.
/// The account id does not change, and the master wrap is left as it is.
///
/// `Ok(true)` means there was no recovery phrase on disk, or it was resealed with
/// the new password. `Ok(false)` means the phrase is sealed with a different
/// password and was left unchanged.
pub fn rewrap_local(
    db: &Path,
    old_password: &str,
    new_password: &str,
    params: Argon2Params,
) -> Result<bool> {
    super::password::check_password(new_password)?;
    set_stage("Checking the current password");
    let (local, old) = open_local(&bootstrap_path(db), old_password)?;
    let dek = load_passphrase_dek(db, &local, old_password, &old)?;
    let fresh = Stretch::fresh(new_password, params)?;
    let home = parent_home(db)?;
    let (phrase_follows, previous_seal) = match super::recovery::reseal(home, old_password, &fresh)?
    {
        super::recovery::Reseal::Absent => (true, None),
        super::recovery::Reseal::Moved { previous } => (true, Some(previous)),
        super::recovery::Reseal::Kept => (false, None),
    };
    set_stage("Saving the new password");
    let boot = bootstrap_path(db);
    let previous = std::fs::read(&boot).with_context(|| format!("reading {}", boot.display()))?;
    if let Err(e) = write_bootstrap(db, &fresh, &local) {
        if let Some(bytes) = previous_seal.as_deref() {
            let _ = super::recovery::restore_bytes(home, bytes);
        }
        return Err(e);
    }
    if let Some((epoch, dek)) = dek
        && let Err(e) = store_passphrase_wrap(db, &local, epoch, &fresh, dek.as_slice())
    {
        let _ = std::fs::write(&boot, previous);
        if let Some(bytes) = previous_seal.as_deref() {
            let _ = super::recovery::restore_bytes(home, bytes);
        }
        return Err(e);
    }
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), local);
    Ok(phrase_follows)
}

/// An encrypted database with the fast Argon2 parameters. Unit tests only.
#[cfg(test)]
pub(crate) fn create_fast(db: &Path, password: &str) -> Result<()> {
    create_new(db, password, Argon2Params::FAST)
}

fn create_new(db: &Path, password: &str, params: Argon2Params) -> Result<()> {
    if let Some(dir) = db.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let stretch = Stretch::fresh(password, params)?;
    let local = random32();
    set_stage("Creating the database");
    write_bootstrap(db, &stretch, &local)?;
    let conn = rusqlite::Connection::open(db)?;
    if let Err(e) = apply_key(&conn, &local) {
        let _ = std::fs::remove_file(bootstrap_path(db));
        let _ = std::fs::remove_file(db);
        return Err(e);
    }
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), local);
    ensure_tables(&conn)?;
    set_stage("Saving the account key");
    seal_epoch(db, &stretch)?;
    // The first encrypted database is the moment to explain the recovery phrase.
    // Migrating a plaintext file and restoring a backup do not come through here.
    super::recovery::mark_next(parent_home(db)?)?;
    Ok(())
}

fn seal_epoch(db: &Path, stretch: &Stretch) -> Result<()> {
    let conn = rusqlite::Connection::open(db)?;
    apply_if_needed(&conn, db)?;
    ensure_tables(&conn)?;
    if current_epoch(&conn)?.is_some() {
        return Ok(());
    }
    let dek = random32();
    let mut wraps = vec![seal_passphrase(stretch, dek.as_slice())?];
    if let Ok(Some(pin)) = Pin::load(parent_home(db)?) {
        wraps.push(seal_master(&pin.wrap_pub, dek.as_slice())?);
    }
    let blob = encode_wraps(&wraps)?;
    conn.execute(
        "INSERT INTO key_epochs (epoch, created_at, reason, issuer, wraps) VALUES (1, ?1, 'created', 'local', ?2)",
        rusqlite::params![now_secs().to_string(), blob],
    )?;
    Ok(())
}

fn export_encrypted(src: &Path, dest: &Path, key: &[u8; 32]) -> Result<()> {
    let from =
        rusqlite::Connection::open(src).with_context(|| format!("opening {}", src.display()))?;
    let hex = data_encoding::HEXLOWER.encode(key);
    let path = dest.display().to_string().replace('\'', "''");
    // The attached file is a temp copy. One fsync after the export is enough;
    // the plaintext file stays in place until the copy has been checked.
    // The raw key skips SQLCipher's password KDF. Do not set kdf_iter.
    let copied = from.execute_batch(&format!(
        "ATTACH DATABASE '{path}' AS encrypted KEY \"x'{hex}'\";
         PRAGMA encrypted.synchronous = OFF;
         PRAGMA encrypted.journal_mode = OFF;
         SELECT sqlcipher_export('encrypted');
         DETACH DATABASE encrypted;"
    ));
    if let Err(e) = copied {
        let _ = std::fs::remove_file(dest);
        return Err(e).context("copying the database into an encrypted file");
    }
    std::fs::File::open(dest)
        .and_then(|f| f.sync_all())
        .context("flushing the encrypted database")?;
    Ok(())
}

fn apply_key(conn: &rusqlite::Connection, key: &[u8; 32]) -> Result<()> {
    let hex = data_encoding::HEXLOWER.encode(key);
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";"))
        .context("setting the database key")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .context("opening the encrypted database")?;
    Ok(())
}

fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        &format!("SELECT count(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"),
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn ensure_tables(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(KEY_EPOCHS_SCHEMA)?;
    conn.execute_batch(IDENTITY_CERTS_SCHEMA)?;
    conn.execute_batch(REVOCATIONS_SCHEMA)?;
    conn.execute_batch(QUARANTINE_SCHEMA)?;
    conn.execute_batch(BACKUPS_SCHEMA)?;
    Ok(())
}

/// Counts per user table, plus the ordered post text when that table exists.
fn fingerprint(db: &Path, key: Option<&[u8; 32]>) -> Result<String> {
    let conn = rusqlite::Connection::open(db)?;
    if let Some(key) = key {
        apply_key(&conn, key)?;
    }
    let mut names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    names.retain(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    let mut hasher = blake3::Hasher::new();
    for name in &names {
        let n: i64 = conn.query_row(&format!("SELECT count(*) FROM \"{name}\""), [], |r| {
            r.get(0)
        })?;
        hasher.update(name.as_bytes());
        hasher.update(&n.to_le_bytes());
    }
    if names.iter().any(|n| n == "posts") && table_has_column(&conn, "posts", "post_id")? {
        let mut st = conn.prepare(
            "SELECT coalesce(post_id, ''), coalesce(content, '') FROM posts ORDER BY post_id",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, content) = row?;
            hasher.update(id.as_bytes());
            hasher.update(content.as_bytes());
        }
    }
    Ok(data_encoding::HEXLOWER.encode(hasher.finalize().as_bytes()))
}

fn write_bootstrap(db: &Path, stretch: &Stretch, local: &[u8; 32]) -> Result<()> {
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let params = stretch.params();
    let wrapped = seal_with(stretch, &nonce, LOCAL_INFO, local)?;
    let value = Value::Array(vec![
        Value::Integer(BOOT_VERSION.into()),
        Value::Integer(u64::from(params.m_kib).into()),
        Value::Integer(u64::from(params.t).into()),
        Value::Integer(u64::from(params.p).into()),
        Value::Bytes(stretch.salt().to_vec()),
        Value::Bytes(nonce.to_vec()),
        Value::Bytes(wrapped),
    ]);
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&value, &mut buf)?;
    let path = bootstrap_path(db);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, buf)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

struct BootFile {
    params: Argon2Params,
    salt: [u8; 16],
    nonce: [u8; 24],
    wrapped: Vec<u8>,
}

fn read_boot(boot: &Path) -> Result<BootFile> {
    let bytes = std::fs::read(boot).with_context(|| format!("reading {}", boot.display()))?;
    let value: Value = ciborium::de::from_reader(bytes.as_slice()).context("bootstrap")?;
    let Value::Array(a) = value else {
        bail!("bootstrap is not an array");
    };
    if a.len() != 7 {
        bail!("bootstrap has {} fields", a.len());
    }
    let version = int_field(&a[0])?;
    if version > BOOT_VERSION {
        bail!("this bootstrap uses version {version}, and this build reads up to {BOOT_VERSION}");
    }
    let salt = bytes_field(&a[4], 16)?;
    let nonce = bytes_field(&a[5], 24)?;
    let wrapped = match &a[6] {
        Value::Bytes(b) => b.clone(),
        _ => bail!("bootstrap wrap is not bytes"),
    };
    let mut salt_a = [0u8; 16];
    let mut nonce_a = [0u8; 24];
    salt_a.copy_from_slice(&salt);
    nonce_a.copy_from_slice(&nonce);
    Ok(BootFile {
        params: Argon2Params {
            m_kib: int_field(&a[1])? as u32,
            t: int_field(&a[2])? as u32,
            p: int_field(&a[3])? as u32,
        },
        salt: salt_a,
        nonce: nonce_a,
        wrapped,
    })
}

fn open_local(boot: &Path, password: &str) -> Result<(Zeroizing<[u8; 32]>, Stretch)> {
    let file = read_boot(boot)?;
    let stretch = Stretch::from_salt(password, file.salt, file.params)?;
    let plain = open_with(&stretch, &file.nonce, LOCAL_INFO, &file.wrapped)?;
    if plain.len() != 32 {
        bail!("the local key has the wrong length");
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plain);
    Ok((key, stretch))
}

fn unwrap_local(boot: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>> {
    Ok(open_local(boot, password)?.0)
}

struct Wrap {
    kind: u64,
    id: Vec<u8>,
    extra: Vec<u8>,
    body: Vec<u8>,
    argon: Option<Argon2Params>,
}

fn seal_passphrase(stretch: &Stretch, dek: &[u8]) -> Result<Wrap> {
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let body = seal_with(stretch, &nonce, ACCOUNT_PASS_INFO, dek)?;
    Ok(Wrap {
        kind: KIND_PASS,
        id: stretch.salt().to_vec(),
        extra: nonce.to_vec(),
        body,
        argon: Some(stretch.params()),
    })
}

fn seal_master(wrap_pub: &[u8; 32], dek: &[u8]) -> Result<Wrap> {
    let (enc, body) = hpke_seal(wrap_pub, dek)?;
    Ok(Wrap {
        kind: KIND_MASTER,
        id: wrap_pub.to_vec(),
        extra: enc,
        body,
        argon: None,
    })
}

/// Seal to a node encryption key. Not used by the CLI; enrollment is a later stage.
pub fn seal_node(enc_pub: &[u8; 32], dek: &[u8]) -> Result<Vec<u8>> {
    let (enc, body) = hpke_seal(enc_pub, dek)?;
    encode_wraps(&[Wrap {
        kind: KIND_NODE,
        id: enc_pub.to_vec(),
        extra: enc,
        body,
        argon: None,
    }])
}

fn unwrap_account(
    wraps: &[Wrap],
    password: &str,
    master: Option<&Master>,
    known: Option<&Stretch>,
) -> Result<Zeroizing<Vec<u8>>> {
    for wrap in wraps {
        if wrap.kind != KIND_PASS {
            continue;
        }
        if let Ok(plain) = open_passphrase_wrap(wrap, password, known) {
            return Ok(Zeroizing::new(plain));
        }
    }
    if let Some(master) = master {
        for wrap in wraps {
            if wrap.kind == KIND_MASTER && wrap.id.as_slice() == master.wrap_pub {
                return master.open_account_dek(&wrap.extra, &wrap.body);
            }
        }
    }
    bail!("no wrap of the account key opens with this password");
}

fn wraps_to_master(wraps: &[Wrap], wrap_pub: &[u8; 32]) -> bool {
    wraps
        .iter()
        .any(|w| w.kind == KIND_MASTER && w.id.as_slice() == wrap_pub)
}

fn encode_wraps(wraps: &[Wrap]) -> Result<Vec<u8>> {
    let items = wraps
        .iter()
        .map(|w| {
            let mut fields = vec![
                Value::Integer(w.kind.into()),
                Value::Bytes(w.id.clone()),
                Value::Bytes(w.extra.clone()),
                Value::Bytes(w.body.clone()),
            ];
            if let Some(p) = w.argon {
                fields.push(Value::Integer(u64::from(p.m_kib).into()));
                fields.push(Value::Integer(u64::from(p.t).into()));
                fields.push(Value::Integer(u64::from(p.p).into()));
            }
            Value::Array(fields)
        })
        .collect();
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&Value::Array(items), &mut buf)?;
    Ok(buf)
}

fn decode_wraps(blob: &[u8]) -> Result<Vec<Wrap>> {
    let value: Value = ciborium::de::from_reader(blob).context("key wraps")?;
    let Value::Array(items) = value else {
        bail!("key wraps are not a list");
    };
    let mut out = Vec::new();
    for item in items {
        let Value::Array(a) = item else {
            bail!("a key wrap is not an array");
        };
        if a.len() != 4 && a.len() != 7 {
            bail!("a key wrap has {} fields", a.len());
        }
        let argon = if a.len() == 7 {
            Some(Argon2Params {
                m_kib: int_field(&a[4])? as u32,
                t: int_field(&a[5])? as u32,
                p: int_field(&a[6])? as u32,
            })
        } else {
            None
        };
        out.push(Wrap {
            kind: int_field(&a[0])?,
            id: bytes_any(&a[1])?,
            extra: bytes_any(&a[2])?,
            body: bytes_any(&a[3])?,
            argon,
        });
    }
    Ok(out)
}

fn current_epoch(conn: &rusqlite::Connection) -> Result<Option<(i64, Vec<Wrap>)>> {
    let row = conn
        .query_row(
            "SELECT epoch, wraps FROM key_epochs ORDER BY epoch DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)),
        )
        .ok();
    match row {
        Some((epoch, blob)) => Ok(Some((epoch, decode_wraps(&blob)?))),
        None => Ok(None),
    }
}

fn store_wraps(conn: &rusqlite::Connection, epoch: i64, wraps: &[Wrap]) -> Result<()> {
    let blob = encode_wraps(wraps)?;
    conn.execute(
        "UPDATE key_epochs SET wraps = ?1 WHERE epoch = ?2",
        rusqlite::params![blob, epoch],
    )?;
    Ok(())
}

fn seal_with(stretch: &Stretch, nonce: &[u8; 24], info: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    aead_encrypt(&stretch.key(info), nonce, info, plain)
}

fn open_with(stretch: &Stretch, nonce: &[u8; 24], info: &[u8], body: &[u8]) -> Result<Vec<u8>> {
    aead_decrypt(&stretch.key(info), nonce, info, body)
}

fn open_passphrase_wrap(wrap: &Wrap, password: &str, known: Option<&Stretch>) -> Result<Vec<u8>> {
    let Some(params) = wrap.argon else {
        bail!("a passphrase wrap has no parameters");
    };
    if wrap.id.len() != 16 || wrap.extra.len() != 24 {
        bail!("a passphrase wrap has the wrong length");
    }
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    salt.copy_from_slice(&wrap.id);
    nonce.copy_from_slice(&wrap.extra);
    if let Some(known) = known
        && *known.salt() == salt
        && known.params() == params
    {
        return open_with(known, &nonce, ACCOUNT_PASS_INFO, &wrap.body);
    }
    let stretch = Stretch::from_salt(password, salt, params)?;
    open_with(&stretch, &nonce, ACCOUNT_PASS_INFO, &wrap.body)
}

fn load_passphrase_dek(
    db: &Path,
    local: &[u8; 32],
    password: &str,
    old: &Stretch,
) -> Result<Option<(i64, Zeroizing<Vec<u8>>)>> {
    let conn = rusqlite::Connection::open(db)?;
    apply_key(&conn, local)?;
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'key_epochs'",
        [],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(None);
    }
    let Some((epoch, wraps)) = current_epoch(&conn)? else {
        return Ok(None);
    };
    if !wraps.iter().any(|w| w.kind == KIND_PASS) {
        return Ok(None);
    }
    let dek = unwrap_account(&wraps, password, None, Some(old))?;
    Ok(Some((epoch, dek)))
}

fn store_passphrase_wrap(
    db: &Path,
    local: &[u8; 32],
    epoch: i64,
    stretch: &Stretch,
    dek: &[u8],
) -> Result<()> {
    let conn = rusqlite::Connection::open(db)?;
    apply_key(&conn, local)?;
    let Some((_, mut wraps)) = current_epoch(&conn)? else {
        return Ok(());
    };
    wraps.retain(|w| w.kind != KIND_PASS);
    wraps.insert(0, seal_passphrase(stretch, dek)?);
    store_wraps(&conn, epoch, &wraps)
}

fn aead_encrypt(
    key: &Zeroizing<[u8; 32]>,
    nonce: &[u8; 24],
    info: &[u8],
    plain: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .encrypt(
            nonce.into(),
            Payload {
                msg: plain,
                aad: info,
            },
        )
        .map_err(|_| anyhow::anyhow!("wrapping a key failed"))
}

fn aead_decrypt(
    key: &Zeroizing<[u8; 32]>,
    nonce: &[u8; 24],
    info: &[u8],
    body: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: body,
                aad: info,
            },
        )
        .map_err(|_| {
            anyhow::anyhow!("the password does not open this database. Nothing was written.")
        })
}

fn hpke_seal(pk: &[u8; 32], plain: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    acp_rs_node_core::hpke::seal(pk, plain)
}

/// Open an HPKE wrap with a raw X25519 secret.
pub fn hpke_open(sk: &[u8; 32], enc: &[u8], body: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    acp_rs_node_core::hpke::open(sk, enc, body)
}

/// Argon2id parameters for a new wrap of `db`: the pin, `ACP_ARGON_FAST`, or the floor.
pub(crate) fn password_params(db: &Path) -> Result<Argon2Params> {
    chosen_params(db)
}

fn chosen_params(db: &Path) -> Result<Argon2Params> {
    if let Some(pin) = Pin::load(parent_home(db)?)? {
        return Ok(pin.argon2);
    }
    // The end-to-end suite sets this so it does not run the 64 MiB KDF on every
    // scenario. A real account leaves it unset and gets [`super::password::default_params`].
    if std::env::var("ACP_ARGON_FAST").ok().as_deref() == Some("1") {
        return Ok(Argon2Params::FAST);
    }
    Ok(super::password::default_params())
}

fn parent_home(db: &Path) -> Result<&Path> {
    // db is `<home>/.acp/ai/data/rs.database`.
    db.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow::anyhow!("database path {} is not under a home", db.display()))
}

fn int_field(v: &Value) -> Result<u64> {
    match v {
        Value::Integer(i) => i64::try_from(*i)
            .ok()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| anyhow::anyhow!("integer is out of range")),
        _ => bail!("expected an integer"),
    }
}

fn bytes_field(v: &Value, len: usize) -> Result<Vec<u8>> {
    match v {
        Value::Bytes(b) if b.len() == len => Ok(b.clone()),
        _ => bail!("expected {len} bytes"),
    }
}

fn bytes_any(v: &Value) -> Result<Vec<u8>> {
    match v {
        Value::Bytes(b) => Ok(b.clone()),
        _ => bail!("expected bytes"),
    }
}

fn random32() -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, out.as_mut());
    out
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DB_REL_PATH;
    use hpke::Kem as KemTrait;
    use hpke::Serializable;

    const PASSWORD: &str = "correct horse battery staple";

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "acp-enc-{}-{}-{}",
            label,
            std::process::id(),
            now_secs()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn db_of(home: &Path) -> PathBuf {
        home.join(DB_REL_PATH)
    }

    fn seed_plain(db: &Path) {
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let c = rusqlite::Connection::open(db).unwrap();
        c.execute_batch(
            "CREATE TABLE posts (post_id TEXT, content TEXT);
             INSERT INTO posts VALUES ('p1', 'alpha');
             INSERT INTO posts VALUES ('p2', 'beta');",
        )
        .unwrap();
    }

    #[test]
    fn a_plaintext_database_is_told_to_migrate() {
        let home = scratch("legacy");
        let db = db_of(&home);
        assert!(legacy_plaintext_notice(&db).is_none());
        seed_plain(&db);
        let msg = legacy_plaintext_notice(&db).unwrap();
        assert!(msg.contains(db.to_str().unwrap()), "{msg}");
        assert!(msg.contains("--encrypt-db"), "{msg}");
        assert!(msg.contains("2.x"), "{msg}");
        encrypt_in_place(&db, PASSWORD, Argon2Params::FAST).unwrap();
        assert!(legacy_plaintext_notice(&db).is_none());
        assert!(!home.join(crate::config::RECOVERY_NEXT_REL_PATH).is_file());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_new_database_is_encrypted_and_a_wrong_password_is_refused() {
        let home = scratch("new");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        assert!(bootstrap_path(&db).is_file());
        assert!(home.join(crate::config::RECOVERY_NEXT_REL_PATH).is_file());
        assert!(!is_plaintext_db(&db));
        unlock(&db, PASSWORD).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        apply_if_needed(&conn, &db).unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS posts (post_id TEXT, content TEXT);")
            .unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM key_epochs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let err = unwrap_local(&bootstrap_path(&db), "correct horse battery staple!")
            .unwrap_err()
            .to_string();
        assert!(err.contains("password"), "{err}");
        assert!(!err.contains("correct horse battery staple!"), "{err}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn encrypt_db_matches_counts_and_text_and_removes_the_plaintext_file() {
        let home = scratch("mig");
        let db = db_of(&home);
        seed_plain(&db);
        let backup = home.join("before.acpbackup");
        crate::crypto::backup::write_backup(&home, &backup, PASSWORD, Argon2Params::FAST).unwrap();
        encrypt_in_place(&db, PASSWORD, Argon2Params::FAST).unwrap();
        assert!(backup.is_file());
        assert!(!is_plaintext_db(&db));
        assert!(!PathBuf::from(format!("{}.prev", db.display())).exists());
        unlock(&db, PASSWORD).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        apply_if_needed(&conn, &db).unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM posts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let text: String = conn
            .query_row("SELECT content FROM posts WHERE post_id = 'p1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(text, "alpha");
        let epochs: i64 = conn
            .query_row("SELECT count(*) FROM key_epochs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(epochs, 1);
        let certs: i64 = conn
            .query_row("SELECT count(*) FROM identity_certs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(certs, 0);
        let after = home.join("after.acpbackup");
        crate::crypto::backup::write_backup(&home, &after, PASSWORD, Argon2Params::FAST).unwrap();
        let header = crate::crypto::backup::inspect(&after).unwrap();
        assert_eq!(header.epochs, vec![1]);
        let fresh = scratch("mig-fresh");
        crate::crypto::backup::restore_backup(&fresh, &after, PASSWORD, None).unwrap();
        let restored = db_of(&fresh);
        assert!(!is_plaintext_db(&restored));
        unlock(&restored, PASSWORD).unwrap();
        let conn = rusqlite::Connection::open(&restored).unwrap();
        apply_if_needed(&conn, &restored).unwrap();
        let text: String = conn
            .query_row("SELECT content FROM posts WHERE post_id = 'p1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(text, "alpha");
        let epochs: i64 = conn
            .query_row("SELECT count(*) FROM key_epochs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(epochs, 1);
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    #[test]
    fn the_master_and_a_node_can_open_the_account_key() {
        let entropy = [0x11u8; 32];
        let phrase = super::super::master::phrase_from_entropy(&entropy).unwrap();
        let master =
            super::super::master::derive_master(&phrase, PASSWORD, Argon2Params::FAST).unwrap();
        let dek = *random32();
        let wrap = seal_master(&master.wrap_pub, &dek).unwrap();
        let opened = master.open_account_dek(&wrap.extra, &wrap.body).unwrap();
        assert_eq!(opened.as_slice(), &dek);

        let mut rng = rand::rngs::OsRng;
        let (sk, pk) = <HpkeKem as KemTrait>::gen_keypair(&mut rng);
        let pk_bytes = pk.to_bytes();
        let mut node_pub = [0u8; 32];
        node_pub.copy_from_slice(pk_bytes.as_slice());
        let blob = seal_node(&node_pub, &dek).unwrap();
        let wraps = decode_wraps(&blob).unwrap();
        assert_eq!(wraps[0].kind, KIND_NODE);
        let sk_bytes = sk.to_bytes();
        let mut node_sk = [0u8; 32];
        node_sk.copy_from_slice(sk_bytes.as_slice());
        let opened = hpke_open(&node_sk, &wraps[0].extra, &wraps[0].body).unwrap();
        assert_eq!(opened.as_slice(), &dek);
    }

    #[test]
    fn rewrapping_the_local_key_does_not_need_the_master() {
        let home = scratch("rewrap");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        let next = "correct horse battery staple and more";
        rewrap_local(&db, PASSWORD, next, Argon2Params::FAST).unwrap();
        assert!(unwrap_local(&bootstrap_path(&db), PASSWORD).is_err());
        unlock(&db, next).unwrap();
        let dek = account_dek(&db, next);
        assert_eq!(dek.len(), 32);
        assert!(account_dek_err(&db, PASSWORD).contains("password"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn creating_or_migrating_stretches_the_password_once() {
        let home = scratch("once");
        let db = db_of(&home);
        super::super::password::start_argon_count();
        super::super::progress::record_stages();
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        assert_eq!(super::super::password::argon_count(), 1);
        let stages = super::super::progress::stages();
        assert_eq!(
            stages
                .iter()
                .filter(|s| s.as_str() == "Deriving key")
                .count(),
            1
        );
        assert!(stages.iter().any(|s| s.as_str() == "Creating the database"));

        let plain = scratch("once-plain");
        let db = db_of(&plain);
        seed_plain(&db);
        let backup = plain.join("before.acpbackup");
        super::super::password::start_argon_count();
        super::super::progress::record_stages();
        encrypt_db(&db, PASSWORD, &backup).unwrap();
        assert_eq!(
            super::super::password::argon_count(),
            1,
            "backup, local key, and account key share one stretch"
        );
        let stages = super::super::progress::stages();
        assert_eq!(
            stages
                .iter()
                .filter(|s| s.as_str() == "Deriving key")
                .count(),
            1
        );
        assert!(
            stages
                .iter()
                .any(|s| s.as_str() == "Snapshotting the database")
        );
        assert!(stages.iter().any(|s| s.as_str() == "Encrypting database"));
        assert!(stages.iter().any(|s| s.as_str() == "Verifying"));
        let fresh = scratch("once-restore");
        super::super::password::start_argon_count();
        super::super::progress::record_stages();
        crate::crypto::backup::restore_backup(&fresh, &backup, PASSWORD, None).unwrap();
        assert_eq!(super::super::password::argon_count(), 1);
        let stages = super::super::progress::stages();
        assert!(stages.iter().any(|s| s.as_str() == "Restoring files"));
        assert!(stages.iter().any(|s| s.as_str() == "Encrypting database"));
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&plain);
        let _ = std::fs::remove_dir_all(&fresh);
    }

    #[test]
    fn backing_up_an_encrypted_database_reuses_the_unlock_stretch() {
        let home = scratch("backup-once");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        super::super::password::start_argon_count();
        let stretch = unlock_stretch(&db, PASSWORD).unwrap();
        let file = home.join("once.acpbackup");
        crate::crypto::backup::write_backup_with(&home, &file, &stretch).unwrap();
        assert_eq!(super::super::password::argon_count(), 1);
        assert!(file.is_file());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_password_change_stretches_each_password_once_when_the_salts_match() {
        let home = scratch("rewrap-once");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        let before = account_dek(&db, PASSWORD);
        let next = "correct horse battery staple and more";
        super::super::password::start_argon_count();
        super::super::progress::record_stages();
        rewrap_local(&db, PASSWORD, next, Argon2Params::FAST).unwrap();
        assert_eq!(super::super::password::argon_count(), 2);
        let stages = super::super::progress::stages();
        assert_eq!(
            stages
                .iter()
                .filter(|s| s.as_str() == "Deriving key")
                .count(),
            2
        );
        assert!(
            stages
                .iter()
                .any(|s| s.as_str() == "Saving the new password")
        );
        assert_eq!(account_dek(&db, next), before);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_password_change_reseals_a_phrase_that_used_the_same_password() {
        let home = scratch("rewrap-seal");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        let entropy = [9u8; 32];
        super::super::recovery::seal(&home, PASSWORD, Argon2Params::FAST, &entropy).unwrap();
        let next = "correct horse battery staple and more";
        super::super::password::start_argon_count();
        assert!(rewrap_local(&db, PASSWORD, next, Argon2Params::FAST).unwrap());
        // Old password (bootstrap), new password, old password again (the seal's own salt).
        assert_eq!(super::super::password::argon_count(), 3);
        assert_eq!(
            super::super::recovery::open(&home, next)
                .unwrap()
                .as_slice(),
            &entropy
        );
        assert!(super::super::recovery::open(&home, PASSWORD).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_phrase_sealed_with_another_password_is_left_alone() {
        let home = scratch("rewrap-other");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        let entropy = [8u8; 32];
        let other = "a different passphrase for the phrase";
        super::super::recovery::seal(&home, other, Argon2Params::FAST, &entropy).unwrap();
        let next = "correct horse battery staple and more";
        assert!(!rewrap_local(&db, PASSWORD, next, Argon2Params::FAST).unwrap());
        assert_eq!(
            super::super::recovery::open(&home, other)
                .unwrap()
                .as_slice(),
            &entropy
        );
        assert!(super::super::recovery::open(&home, next).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn an_account_wrap_with_its_own_salt_still_opens() {
        let home = scratch("oldsalt");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        unlock(&db, PASSWORD).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        apply_if_needed(&conn, &db).unwrap();
        let (epoch, mut wraps) = current_epoch(&conn).unwrap().unwrap();
        let dek = unwrap_account(&wraps, PASSWORD, None, None).unwrap();
        let other = Stretch::fresh(PASSWORD, Argon2Params::FAST).unwrap();
        wraps.retain(|w| w.kind != KIND_PASS);
        wraps.insert(0, seal_passphrase(&other, dek.as_slice()).unwrap());
        store_wraps(&conn, epoch, &wraps).unwrap();
        drop(conn);
        super::super::password::start_argon_count();
        let opened = {
            let conn = rusqlite::Connection::open(&db).unwrap();
            apply_if_needed(&conn, &db).unwrap();
            let (_, wraps) = current_epoch(&conn).unwrap().unwrap();
            unwrap_account(&wraps, PASSWORD, None, None).unwrap()
        };
        assert_eq!(opened.as_slice(), dek.as_slice());
        assert_eq!(super::super::password::argon_count(), 1);
        let next = "correct horse battery staple and more";
        super::super::password::start_argon_count();
        rewrap_local(&db, PASSWORD, next, Argon2Params::FAST).unwrap();
        assert_eq!(super::super::password::argon_count(), 3);
        assert_eq!(account_dek(&db, next), dek.to_vec());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_raw_sqlcipher_key_does_not_run_the_password_kdf() {
        use std::time::Instant;
        let dir = scratch("kdf");
        let raw_path = dir.join("raw.db");
        let pass_path = dir.join("pass.db");
        let key = [3u8; 32];
        let hex = data_encoding::HEXLOWER.encode(&key);
        let raw = rusqlite::Connection::open(&raw_path).unwrap();
        raw.execute_batch(&format!(
            "PRAGMA key = \"x'{hex}'\"; CREATE TABLE t (x); INSERT INTO t VALUES (1);"
        ))
        .unwrap();
        let kdf: String = raw.query_row("PRAGMA kdf_iter", [], |r| r.get(0)).unwrap();
        let algo: String = raw
            .query_row("PRAGMA cipher_kdf_algorithm", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            kdf, "256000",
            "the password KDF stays at the SQLCipher default"
        );
        assert_eq!(algo, "PBKDF2_HMAC_SHA512");
        drop(raw);
        let started = Instant::now();
        let raw = rusqlite::Connection::open(&raw_path).unwrap();
        apply_key(&raw, &key).unwrap();
        let _: i64 = raw.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        let raw_open = started.elapsed();

        let pass = rusqlite::Connection::open(&pass_path).unwrap();
        pass.execute_batch(
            "PRAGMA key = 'correct horse battery staple'; CREATE TABLE t (x); INSERT INTO t VALUES (1);",
        )
        .unwrap();
        drop(pass);
        let started = Instant::now();
        let pass = rusqlite::Connection::open(&pass_path).unwrap();
        pass.execute_batch("PRAGMA key = 'correct horse battery staple';")
            .unwrap();
        let _: i64 = pass.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        let pass_open = started.elapsed();
        assert!(raw_open.as_millis() < 200, "raw key open took {raw_open:?}");
        assert!(
            pass_open.as_millis() > raw_open.as_millis().saturating_mul(3).max(5),
            "passphrase open {pass_open:?} should show the KDF the raw key skips (raw {raw_open:?})"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unlocked_database_opens_again_without_a_password() {
        let home = scratch("session");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        let key = cached_key(&db).expect("creating the database caches its key");
        forget(&db);
        let err = prepare(&db, None).unwrap_err().to_string();
        assert!(err.contains("password"), "{err}");
        if std::env::var_os("ACP_PASSWORD").is_none() {
            let err = crate::storage::db::open_db(&db).unwrap_err().to_string();
            assert!(err.contains("ACP_PASSWORD"), "{err}");
        }
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            let err = crate::crypto::cmd::prepare_path(&db).unwrap_err().to_string();
            assert!(
                err.contains(
                    "this database needs a password. Set ACP_PASSWORD, or run from a terminal."
                ),
                "{err}"
            );
        }
        install_cached_key(&db, &key);
        prepare(&db, None).unwrap();
        crate::crypto::cmd::prepare_path(&db).unwrap();
        let conn = crate::storage::db::open_db(&db).unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM key_epochs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        forget(&db);
        unlock(&db, PASSWORD).unwrap();
        prepare(&db, None).unwrap();
        let _ = std::fs::remove_dir_all(&home);
        forget(&db);
    }

    fn account_dek(db: &Path, password: &str) -> Vec<u8> {
        let conn = rusqlite::Connection::open(db).unwrap();
        apply_if_needed(&conn, db).unwrap();
        let (_, wraps) = current_epoch(&conn).unwrap().unwrap();
        unwrap_account(&wraps, password, None, None)
            .unwrap()
            .to_vec()
    }

    fn account_dek_err(db: &Path, password: &str) -> String {
        let conn = rusqlite::Connection::open(db).unwrap();
        apply_if_needed(&conn, db).unwrap();
        let (_, wraps) = current_epoch(&conn).unwrap().unwrap();
        unwrap_account(&wraps, password, None, None)
            .unwrap_err()
            .to_string()
    }
}
