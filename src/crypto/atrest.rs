//! Encryption at rest.
//!
//! The on-disk database is SQLCipher. Its key is a random 256-bit local key, passed
//! with `PRAGMA key = "x'<hex>'"` so SQLCipher does not run its own password KDF.
//! That local key is wrapped with XChaCha20-Poly1305 under HKDF(Argon2id(password))
//! and stored in a bootstrap file next to the database. The bootstrap cannot live
//! inside the database: the database cannot be opened without it.
//!
//! A separate account data key, one per epoch, is what a lost node would rotate.
//! It is wrapped to the local password and, when an `acp-id` is pinned, to the
//! master wrapping key with HPKE (X25519, HKDF-SHA256, ChaCha20-Poly1305). A node
//! recipient is accepted by the same decoder and is not produced here.
//!
//! An existing plaintext database stays readable until `--encrypt-db`. A database
//! this program creates is encrypted. Callers that never ask for a password, including
//! the unit tests, still get a plaintext file.

use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use ciborium::Value;
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use zeroize::Zeroizing;

use super::master::Master;
use super::password::{Argon2Params, argon2id};
use super::pin::Pin;
use crate::storage::schema::{
    BACKUPS_SCHEMA, IDENTITY_CERTS_SCHEMA, KEY_EPOCHS_SCHEMA, QUARANTINE_SCHEMA, REVOCATIONS_SCHEMA,
};

const BOOT_VERSION: u64 = 1;
const LOCAL_INFO: &[u8] = b"acp-local-key-v1";
const ACCOUNT_PASS_INFO: &[u8] = b"acp-account-pass-v1";
const ACCOUNT_HPKE_INFO: &[u8] = b"acp-account-dek-v1";
const KIND_PASS: u64 = 0;
const KIND_MASTER: u64 = 1;
const KIND_NODE: u64 = 2;

type HpkeKem = X25519HkdfSha256;
type AeadHpke = ChaCha20Poly1305;
type Kdf = HkdfSha256;

fn cache() -> &'static Mutex<HashMap<PathBuf, Zeroizing<[u8; 32]>>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<PathBuf, Zeroizing<[u8; 32]>>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(db: &Path) -> PathBuf {
    std::fs::canonicalize(db).unwrap_or_else(|_| db.to_path_buf())
}

/// Bootstrap file for `db` (`rs.database.bootstrap`).
pub fn bootstrap_path(db: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bootstrap", db.display()))
}

/// Whether `path` is a plaintext SQLite file.
pub fn is_plaintext_db(path: &Path) -> bool {
    header(path).is_some_and(|b| &b == b"SQLite format 3\0")
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
    let key = unwrap_local(&bootstrap_path(db), password)?;
    let conn =
        rusqlite::Connection::open(db).with_context(|| format!("opening {}", db.display()))?;
    apply_key(&conn, &key)?;
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), key);
    Ok(())
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
    super::backup::write_backup(home, backup_to, password, params)
        .context("backup before encryption")?;
    encrypt_in_place(db, password, params)
}

/// Replace a plaintext database at `db` with an encrypted one. No backup.
pub fn encrypt_in_place(db: &Path, password: &str, params: Argon2Params) -> Result<()> {
    if bootstrap_path(db).is_file() || !is_plaintext_db(db) {
        bail!("the database is already encrypted");
    }
    let before = fingerprint(db, None)?;
    let local = random32();
    let tmp = PathBuf::from(format!("{}.encrypting", db.display()));
    let _ = std::fs::remove_file(&tmp);
    export_encrypted(db, &tmp, &local)?;
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
    if let Err(e) = write_bootstrap(db, password, params, &local) {
        let _ = std::fs::rename(db, &tmp);
        let _ = std::fs::rename(&prev, db);
        return Err(e);
    }
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), local.clone());
    let check = fingerprint(db, Some(&local))?;
    if check != before {
        bail!(
            "the installed database does not match; {} was kept",
            prev.display()
        );
    }
    seal_epoch(db, password, params)?;
    let _ = std::fs::remove_file(&prev);
    Ok(())
}

/// Add a master HPKE wrap of the current account key, if a database and an epoch exist.
pub fn add_master_recipient(db: &Path, password: &str, master: &Master) -> Result<()> {
    if !bootstrap_path(db).is_file() {
        return Ok(());
    }
    unlock(db, password)?;
    let conn = rusqlite::Connection::open(db)?;
    apply_if_needed(&conn, db)?;
    ensure_tables(&conn)?;
    let Some((epoch, wraps)) = current_epoch(&conn)? else {
        return Ok(());
    };
    let dek = unwrap_account(&wraps, password, Some(master))?;
    if wraps_to_master(&wraps, &master.wrap_pub) {
        return Ok(());
    }
    let mut next = wraps;
    next.push(seal_master(&master.wrap_pub, &dek)?);
    store_wraps(&conn, epoch, &next)?;
    Ok(())
}

/// Rewrap the local key with a new password. The account id does not change.
pub fn rewrap_local(
    db: &Path,
    old_password: &str,
    new_password: &str,
    params: Argon2Params,
) -> Result<()> {
    super::password::check_password(new_password)?;
    let local = unwrap_local(&bootstrap_path(db), old_password)?;
    write_bootstrap(db, new_password, params, &local)?;
    cache()
        .lock()
        .expect("key cache")
        .insert(cache_key(db), local);
    Ok(())
}

fn create_new(db: &Path, password: &str, params: Argon2Params) -> Result<()> {
    if let Some(dir) = db.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let local = random32();
    write_bootstrap(db, password, params, &local)?;
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
    seal_epoch(db, password, params)?;
    Ok(())
}

fn seal_epoch(db: &Path, password: &str, params: Argon2Params) -> Result<()> {
    let conn = rusqlite::Connection::open(db)?;
    apply_if_needed(&conn, db)?;
    ensure_tables(&conn)?;
    if current_epoch(&conn)?.is_some() {
        return Ok(());
    }
    let dek = random32();
    let mut wraps = vec![seal_passphrase(password, params, &dek)?];
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
    from.execute_batch(&format!(
        "ATTACH DATABASE '{path}' AS encrypted KEY \"x'{hex}'\";
         SELECT sqlcipher_export('encrypted');
         DETACH DATABASE encrypted;"
    ))
    .context("copying the database into an encrypted file")?;
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

fn write_bootstrap(
    db: &Path,
    password: &str,
    params: Argon2Params,
    local: &[u8; 32],
) -> Result<()> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let wrapped = aead_seal(password, &salt, &nonce, LOCAL_INFO, params, local)?;
    let value = Value::Array(vec![
        Value::Integer(BOOT_VERSION.into()),
        Value::Integer(u64::from(params.m_kib).into()),
        Value::Integer(u64::from(params.t).into()),
        Value::Integer(u64::from(params.p).into()),
        Value::Bytes(salt.to_vec()),
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

fn unwrap_local(boot: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>> {
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
    let params = Argon2Params {
        m_kib: int_field(&a[1])? as u32,
        t: int_field(&a[2])? as u32,
        p: int_field(&a[3])? as u32,
    };
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
    let plain = aead_open(password, &salt_a, &nonce_a, LOCAL_INFO, params, &wrapped)?;
    if plain.len() != 32 {
        bail!("the local key has the wrong length");
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plain);
    Ok(key)
}

struct Wrap {
    kind: u64,
    id: Vec<u8>,
    extra: Vec<u8>,
    body: Vec<u8>,
    argon: Option<Argon2Params>,
}

fn seal_passphrase(password: &str, params: Argon2Params, dek: &[u8; 32]) -> Result<Wrap> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let body = aead_seal(password, &salt, &nonce, ACCOUNT_PASS_INFO, params, dek)?;
    Ok(Wrap {
        kind: KIND_PASS,
        id: salt.to_vec(),
        extra: nonce.to_vec(),
        body,
        argon: Some(params),
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
) -> Result<Zeroizing<Vec<u8>>> {
    for wrap in wraps {
        if wrap.kind != KIND_PASS {
            continue;
        }
        let Some(params) = wrap.argon else { continue };
        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        if wrap.id.len() != 16 || wrap.extra.len() != 24 {
            continue;
        }
        salt.copy_from_slice(&wrap.id);
        nonce.copy_from_slice(&wrap.extra);
        if let Ok(plain) = aead_open(
            password,
            &salt,
            &nonce,
            ACCOUNT_PASS_INFO,
            params,
            &wrap.body,
        ) {
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

fn aead_seal(
    password: &str,
    salt: &[u8; 16],
    nonce: &[u8; 24],
    info: &[u8],
    params: Argon2Params,
    plain: &[u8],
) -> Result<Vec<u8>> {
    let key = derived_key(password, salt, info, params)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
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

fn aead_open(
    password: &str,
    salt: &[u8; 16],
    nonce: &[u8; 24],
    info: &[u8],
    params: Argon2Params,
    body: &[u8],
) -> Result<Vec<u8>> {
    let key = derived_key(password, salt, info, params)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
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

fn derived_key(
    password: &str,
    salt: &[u8],
    info: &[u8],
    params: Argon2Params,
) -> Result<Zeroizing<[u8; 32]>> {
    let argon = Zeroizing::new(argon2id(password, salt, params)?);
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(salt), argon.as_slice());
    let mut key = Zeroizing::new([0u8; 32]);
    hk.expand(info, key.as_mut())
        .expect("32-byte HKDF output is valid");
    Ok(key)
}

fn hpke_seal(pk: &[u8; 32], plain: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let public = <HpkeKem as KemTrait>::PublicKey::from_bytes(pk)
        .map_err(|e| anyhow::anyhow!("wrapping key: {e}"))?;
    let mut rng = rand::rngs::OsRng;
    let (enc, mut ctx) = hpke::setup_sender::<AeadHpke, Kdf, HpkeKem, _>(
        &OpModeS::Base,
        &public,
        ACCOUNT_HPKE_INFO,
        &mut rng,
    )
    .map_err(|e| anyhow::anyhow!("hpke seal: {e}"))?;
    let body = ctx
        .seal(plain, b"")
        .map_err(|e| anyhow::anyhow!("hpke seal: {e}"))?;
    Ok((enc.to_bytes().to_vec(), body))
}

/// Open an HPKE wrap with a raw X25519 secret.
pub fn hpke_open(sk: &[u8; 32], enc: &[u8], body: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let secret = <HpkeKem as KemTrait>::PrivateKey::from_bytes(sk)
        .map_err(|e| anyhow::anyhow!("wrapping key: {e}"))?;
    let encapped = <HpkeKem as KemTrait>::EncappedKey::from_bytes(enc)
        .map_err(|e| anyhow::anyhow!("hpke: {e}"))?;
    let mut ctx = hpke::setup_receiver::<AeadHpke, Kdf, HpkeKem>(
        &OpModeR::Base,
        &secret,
        &encapped,
        ACCOUNT_HPKE_INFO,
    )
    .map_err(|e| anyhow::anyhow!("hpke open: {e}"))?;
    let plain = ctx
        .open(body, b"")
        .map_err(|_| anyhow::anyhow!("hpke open failed"))?;
    Ok(Zeroizing::new(plain))
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
    fn a_new_database_is_encrypted_and_a_wrong_password_is_refused() {
        let home = scratch("new");
        let db = db_of(&home);
        create_new(&db, PASSWORD, Argon2Params::FAST).unwrap();
        assert!(bootstrap_path(&db).is_file());
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
        let _ = std::fs::remove_dir_all(&home);
    }
}
