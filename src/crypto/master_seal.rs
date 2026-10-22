//! The existing master secret, sealed under the database password.
//!
//! A password change does not derive a new master. The first change derives the
//! current one from the phrase and the old password, checks it against the pin,
//! and seals that secret. Later changes only move the seal. A new machine can
//! then check the account id without the old password.

use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use ciborium::Value;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use super::master;
use super::password::{Argon2Params, Stretch};
use super::pin::Pin;
use super::recovery::SealPlan;
use crate::config::MASTER_SEAL_REL_PATH;

const VERSION: u64 = 1;
const INFO: &[u8] = b"acp-master-secret-v1";

/// A master seal prepared for a password change, not yet written.
pub(crate) enum Plan {
    /// No pin, or no phrase sealed with this password. Nothing is written.
    Absent,
    /// `next` is written when the change commits. `previous` is `None` the first time.
    Ready { previous: Option<Vec<u8>>, next: Vec<u8> },
}

/// Build the new master seal without writing it.
pub(crate) fn plan(
    home: &Path,
    old_password: &str,
    old: &Stretch,
    fresh: &Stretch,
    phrase: &SealPlan,
) -> Result<Plan> {
    let path = seal_path(home);
    if path.is_file() {
        let previous = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let secret = open_stretch_or_password(home, old, old_password)?;
        return Ok(Plan::Ready {
            previous: Some(previous),
            next: seal_bytes(fresh, &secret)?,
        });
    }
    let Some(entropy) = phrase.entropy() else {
        return Ok(Plan::Absent);
    };
    let Some(pin) = Pin::load(home)? else {
        return Ok(Plan::Absent);
    };
    let words = master::phrase_from_entropy(entropy)?;
    let derived = master::derive_master(&words, old_password, pin.argon2)?;
    derived.require_id(&pin.acp_id)?;
    Ok(Plan::Ready {
        previous: None,
        next: seal_bytes(fresh, derived.secret())?,
    })
}

/// Open the seal with a stretch that already uses its salt.
pub(crate) fn open_with(home: &Path, stretch: &Stretch) -> Result<Zeroizing<[u8; 32]>> {
    let file = read_seal(&seal_path(home))?;
    if file.salt != *stretch.salt() || file.params != stretch.params() {
        bail!("the sealed master does not match this password stretch");
    }
    decrypt_secret(&stretch.key(INFO), &file.nonce, &file.wrapped)
}

fn open_stretch_or_password(
    home: &Path,
    old: &Stretch,
    old_password: &str,
) -> Result<Zeroizing<[u8; 32]>> {
    match open_with(home, old) {
        Ok(secret) => Ok(secret),
        Err(_) => open_password(home, old_password),
    }
}

fn open_password(home: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>> {
    let file = read_seal(&seal_path(home))?;
    let stretch = Stretch::from_salt(password, file.salt, file.params)?;
    decrypt_secret(&stretch.key(INFO), &file.nonce, &file.wrapped)
}

fn seal_path(home: &Path) -> PathBuf {
    home.join(MASTER_SEAL_REL_PATH)
}

fn seal_bytes(stretch: &Stretch, secret: &[u8; 32]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let wrapped = encrypt(&stretch.key(INFO), &nonce, secret)?;
    let params = stretch.params();
    let value = Value::Array(vec![
        Value::Integer(VERSION.into()),
        Value::Integer(u64::from(params.m_kib).into()),
        Value::Integer(u64::from(params.t).into()),
        Value::Integer(u64::from(params.p).into()),
        Value::Bytes(stretch.salt().to_vec()),
        Value::Bytes(nonce.to_vec()),
        Value::Bytes(wrapped),
    ]);
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&value, &mut buf).context("encoding the sealed master")?;
    Ok(buf)
}

struct SealFile {
    params: Argon2Params,
    salt: [u8; 16],
    nonce: [u8; 24],
    wrapped: Vec<u8>,
}

fn read_seal(path: &Path) -> Result<SealFile> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value = ciborium::de::from_reader(bytes.as_slice()).context("sealed master")?;
    let Value::Array(a) = value else {
        bail!("the sealed master is not an array");
    };
    if a.len() != 7 {
        bail!("the sealed master has {} fields", a.len());
    }
    let version = int_field(&a[0])?;
    if version > VERSION {
        bail!("this sealed master uses version {version}, and this build reads up to {VERSION}");
    }
    let salt = bytes_field(&a[4], 16)?;
    let nonce = bytes_field(&a[5], 24)?;
    let wrapped = match &a[6] {
        Value::Bytes(b) => b.clone(),
        _ => bail!("the sealed master wrap is not bytes"),
    };
    let mut salt_a = [0u8; 16];
    let mut nonce_a = [0u8; 24];
    salt_a.copy_from_slice(&salt);
    nonce_a.copy_from_slice(&nonce);
    Ok(SealFile {
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

fn decrypt_secret(
    key: &Zeroizing<[u8; 32]>,
    nonce: &[u8; 24],
    body: &[u8],
) -> Result<Zeroizing<[u8; 32]>> {
    let plain = decrypt(key, nonce, body)?;
    if plain.len() != 32 {
        bail!("the sealed master has the wrong length");
    }
    let mut secret = Zeroizing::new([0u8; 32]);
    secret.copy_from_slice(&plain);
    Ok(secret)
}

fn encrypt(key: &Zeroizing<[u8; 32]>, nonce: &[u8; 24], plain: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .encrypt(nonce.into(), Payload { msg: plain, aad: INFO })
        .map_err(|_| anyhow::anyhow!("sealing the master failed"))
}

fn decrypt(key: &Zeroizing<[u8; 32]>, nonce: &[u8; 24], body: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .decrypt(nonce.into(), Payload { msg: body, aad: INFO })
        .map_err(|_| anyhow::anyhow!("the password does not open the sealed master. Nothing was written."))
}

fn int_field(v: &Value) -> Result<u64> {
    match v {
        Value::Integer(i) => i64::try_from(*i)
            .ok()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| anyhow::anyhow!("sealed master integer is out of range")),
        _ => bail!("sealed master field is not an integer"),
    }
}

fn bytes_field(v: &Value, n: usize) -> Result<Vec<u8>> {
    match v {
        Value::Bytes(b) if b.len() == n => Ok(b.clone()),
        Value::Bytes(b) => bail!("sealed master field has {} bytes, want {n}", b.len()),
        _ => bail!("sealed master field is not bytes"),
    }
}
