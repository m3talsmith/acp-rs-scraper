//! HPKE wraps of an account data key (X25519, HKDF-SHA256, ChaCha20-Poly1305).
//!
//! The info string is part of the on-disk wrap. It does not change.

use anyhow::{Result, anyhow};
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable};
use zeroize::Zeroizing;

const INFO: &[u8] = b"acp-account-dek-v1";

type Kem = X25519HkdfSha256;
type Aead = ChaCha20Poly1305;
type Kdf = HkdfSha256;

/// Seal `plain` to an X25519 public key. Returns `(encapped, ciphertext)`.
pub fn seal(pk: &[u8; 32], plain: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let public = <Kem as KemTrait>::PublicKey::from_bytes(pk)
        .map_err(|e| anyhow!("wrapping key: {e}"))?;
    let mut rng = rand::rngs::OsRng;
    let (enc, mut ctx) =
        hpke::setup_sender::<Aead, Kdf, Kem, _>(&OpModeS::Base, &public, INFO, &mut rng)
            .map_err(|e| anyhow!("hpke seal: {e}"))?;
    let body = ctx
        .seal(plain, b"")
        .map_err(|e| anyhow!("hpke seal: {e}"))?;
    Ok((enc.to_bytes().to_vec(), body))
}

/// Open an HPKE wrap with a raw X25519 secret.
pub fn open(sk: &[u8; 32], enc: &[u8], body: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let secret =
        <Kem as KemTrait>::PrivateKey::from_bytes(sk).map_err(|e| anyhow!("wrapping key: {e}"))?;
    let encapped =
        <Kem as KemTrait>::EncappedKey::from_bytes(enc).map_err(|e| anyhow!("hpke: {e}"))?;
    let mut ctx = hpke::setup_receiver::<Aead, Kdf, Kem>(&OpModeR::Base, &secret, &encapped, INFO)
        .map_err(|e| anyhow!("hpke open: {e}"))?;
    let plain = ctx
        .open(body, b"")
        .map_err(|_| anyhow!("hpke open failed"))?;
    Ok(Zeroizing::new(plain))
}
