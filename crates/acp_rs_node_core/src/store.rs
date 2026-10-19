//! Interfaces for a sealed secret, without a database.
//!
//! A relay, a tray, and the terminal client can each keep the bytes somewhere
//! else. The seal itself is XChaCha20-Poly1305 under a 32-byte key the caller
//! already has. This crate does not derive that key from a password.

use anyhow::{Result, anyhow};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// Store and open a named secret. The name is authenticated with the ciphertext.
pub trait SecretStore {
    /// Replace any secret stored under `name`.
    fn put(&mut self, name: &str, plaintext: &[u8]) -> Result<()>;
    /// Open `name`. `Ok(None)` when it is absent. A wrong key is an error.
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>>;
    /// Remove `name`. Returns whether it was present.
    fn delete(&mut self, name: &str) -> Result<bool>;
}

/// In-memory store. Tests use it. A process exit drops the map.
#[derive(Debug)]
pub struct MemorySecretStore {
    key: Zeroizing<[u8; 32]>,
    items: BTreeMap<String, Vec<u8>>,
}

impl MemorySecretStore {
    /// Seal later writes with `key`. The key is zeroized when the store is dropped.
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            key: Zeroizing::new(key),
            items: BTreeMap::new(),
        }
    }
}

impl SecretStore for MemorySecretStore {
    fn put(&mut self, name: &str, plaintext: &[u8]) -> Result<()> {
        let sealed = seal_blob(&self.key, name.as_bytes(), plaintext)?;
        self.items.insert(name.to_string(), sealed);
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let Some(blob) = self.items.get(name) else {
            return Ok(None);
        };
        Ok(Some(open_blob(&self.key, name.as_bytes(), blob)?))
    }

    fn delete(&mut self, name: &str) -> Result<bool> {
        Ok(self.items.remove(name).is_some())
    }
}

/// `nonce (24) || ciphertext`, authenticated with `aad`.
pub fn seal_blob(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let cipher = XChaCha20Poly1305::new(key.into());
    let body = cipher
        .encrypt(nonce.as_slice().into(), Payload { msg: plain, aad })
        .map_err(|_| anyhow!("sealing the secret failed"))?;
    let mut out = Vec::with_capacity(24 + body.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Open a blob from [`seal_blob`]. A wrong key or a short blob is an error.
pub fn open_blob(key: &[u8; 32], aad: &[u8], blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if blob.len() < 24 {
        anyhow::bail!("sealed secret is too short");
    }
    let (nonce, body) = blob.split_at(24);
    let cipher = XChaCha20Poly1305::new(key.into());
    let plain = cipher
        .decrypt(nonce.into(), Payload { msg: body, aad })
        .map_err(|_| anyhow!("the key does not open this secret"))?;
    Ok(Zeroizing::new(plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip_and_wrong_key() {
        let mut store = MemorySecretStore::new([7u8; 32]);
        store.put("node", b"alpha").unwrap();
        let got = store.get("node").unwrap().unwrap();
        assert_eq!(got.as_slice(), b"alpha");
        assert!(store.get("missing").unwrap().is_none());
        let sealed = seal_blob(&[7u8; 32], b"node", b"alpha").unwrap();
        assert!(open_blob(&[8u8; 32], b"node", &sealed).is_err());
        assert!(open_blob(&[7u8; 32], b"other", &sealed).is_err());
        assert!(store.delete("node").unwrap());
        assert!(store.get("node").unwrap().is_none());
    }
}
