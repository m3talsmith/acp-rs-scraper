//! Master secret, the 24-word phrase, and the `acp-id`.
//!
//! ```text
//! seed        = 256 bits shown as 24 BIP-39 words (the entropy, not the BIP-39 seed)
//! argon_salt  = blake3("acp-master" ‖ seed)
//! argon_out   = Argon2id(password, salt = argon_salt)
//! master      = HKDF-SHA256(
//!                 ikm  = seed ‖ argon_out,
//!                 salt = argon_salt,
//!                 info = "acp-master-secret-v1")
//! sign seed   = HKDF-SHA256(master, info = "acp-master-ed25519-v1")
//! wrap seed   = HKDF-SHA256(master, info = "acp-master-x25519-v1")
//! acp-id      = base32(blake3(sign public key) ‖ 3-byte checksum), prefixed `acp1-`
//! ```
//!
//! The email address is not an input. A wrong password still derives a master, so
//! callers compare [`AcpId`] with [`Master::require_id`] and show the derived id.

use anyhow::{Context, Result, bail};
use bip39::{Language, Mnemonic};
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use super::password::{Argon2Params, argon2id};

const MASTER_SALT_LABEL: &[u8] = b"acp-master";
const MASTER_INFO: &[u8] = b"acp-master-secret-v1";
const SIGN_INFO: &[u8] = b"acp-master-ed25519-v1";
const WRAP_INFO: &[u8] = b"acp-master-x25519-v1";
const ID_LABEL: &[u8] = b"acp-id-v1";

/// Account id: blake3 of the master signing public key, base32, with a checksum.
#[derive(Clone, PartialEq, Eq)]
pub struct AcpId(String);

impl AcpId {
    /// The text form (`acp1-` plus 56 base32 characters).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse and check the checksum. The case of the base32 does not matter.
    pub fn parse(s: &str) -> Result<Self> {
        let raw = s.trim();
        let Some(body) = raw
            .get(5..)
            .filter(|_| raw.len() >= 5 && raw[..5].eq_ignore_ascii_case("acp1-"))
        else {
            bail!("an acp-id starts with acp1-");
        };
        let bytes = data_encoding::BASE32_NOPAD
            .decode(body.to_ascii_uppercase().as_bytes())
            .context("acp-id is not base32")?;
        if bytes.len() != 35 {
            bail!("acp-id has the wrong length");
        }
        let (key, check) = bytes.split_at(32);
        let expect = checksum(key);
        if check != expect.as_slice() {
            bail!("acp-id checksum does not match");
        }
        Ok(AcpId(format!(
            "acp1-{}",
            data_encoding::BASE32_NOPAD
                .encode(&bytes)
                .to_ascii_lowercase()
        )))
    }
}

impl std::fmt::Display for AcpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for AcpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AcpId").field(&self.0).finish()
    }
}

fn checksum(public_hash: &[u8]) -> [u8; 3] {
    let mut h = blake3::Hasher::new();
    h.update(ID_LABEL);
    h.update(public_hash);
    let full = h.finalize();
    let mut out = [0u8; 3];
    out.copy_from_slice(&full.as_bytes()[..3]);
    out
}

fn id_from_sign_pub(sign_pub: &[u8; 32]) -> AcpId {
    let raw = blake3::hash(sign_pub);
    let mut body = [0u8; 35];
    body[..32].copy_from_slice(raw.as_bytes());
    body[32..].copy_from_slice(&checksum(raw.as_bytes()));
    AcpId(format!(
        "acp1-{}",
        data_encoding::BASE32_NOPAD
            .encode(&body)
            .to_ascii_lowercase()
    ))
}

/// The master secret and the two keys derived from it. Dropped memory is zeroized.
/// Debug output says `[redacted]` and does not include key bytes.
pub struct Master {
    /// HKDF master secret. Not read after the keys are derived; kept so it is zeroized
    /// and so a later ceremony can use it without re-running Argon2.
    #[allow(dead_code)]
    secret: Zeroizing<[u8; 32]>,
    /// ed25519 seed. Same reason as `secret`. Certificates are a later stage.
    #[allow(dead_code)]
    sign_seed: Zeroizing<[u8; 32]>,
    /// X25519 seed. Same reason as `secret`.
    #[allow(dead_code)]
    wrap_seed: Zeroizing<[u8; 32]>,
    /// `acp-id` of this master.
    pub acp_id: AcpId,
    /// ed25519 verifying key (public).
    pub sign_pub: [u8; 32],
    /// X25519 public key future wraps are sealed to.
    pub wrap_pub: [u8; 32],
}

impl std::fmt::Debug for Master {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Master")
            .field("secret", &"[redacted]")
            .field("acp_id", &self.acp_id)
            .finish()
    }
}

impl Master {
    /// Open an account-key wrap sealed to this master's X25519 key.
    pub fn open_account_dek(
        &self,
        encapped: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        super::atrest::hpke_open(&self.wrap_seed, encapped, ciphertext)
    }

    /// Refuse unless `expected` is this master's id. The error includes the derived
    /// id, because a wrong password still produces one.
    pub fn require_id(&self, expected: &AcpId) -> Result<()> {
        if &self.acp_id == expected {
            return Ok(());
        }
        bail!(
            "this password does not match the pinned account\n\
             derived acp-id: {}\n\
             pinned acp-id:  {}\n\
             A wrong password still derives an id. Nothing was written.",
            self.acp_id,
            expected
        )
    }
}

/// 24 BIP-39 words for 256 bits of entropy.
pub fn phrase_from_entropy(entropy: &[u8; 32]) -> Result<String> {
    let m = Mnemonic::from_entropy_in(Language::English, entropy).context("BIP-39 phrase")?;
    Ok(m.to_string())
}

/// The 256-bit entropy a phrase encodes. Not the BIP-39 seed (that mixes in a passphrase).
pub fn recover_seed(phrase: &str) -> Result<Zeroizing<[u8; 32]>> {
    let joined = normalize_phrase(phrase);
    let m = Mnemonic::parse_in_normalized(Language::English, &joined).context("recovery phrase")?;
    let ent = m.to_entropy();
    if ent.len() != 32 {
        bail!("the recovery phrase must be 24 words (256 bits)");
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    seed.copy_from_slice(&ent);
    Ok(seed)
}

/// Collapse whitespace and case so a retyped phrase matches.
pub fn normalize_phrase(phrase: &str) -> String {
    phrase
        .split_whitespace()
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn hkdf(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out)
        .expect("32-byte HKDF output is valid");
    out
}

/// Derive the master from a phrase and a password. Does not check a pinned id;
/// use [`Master::require_id`] for that.
pub fn derive_master(phrase: &str, password: &str, params: Argon2Params) -> Result<Master> {
    let seed = recover_seed(phrase)?;
    let mut salt_hasher = blake3::Hasher::new();
    salt_hasher.update(MASTER_SALT_LABEL);
    salt_hasher.update(seed.as_slice());
    let argon_salt = *salt_hasher.finalize().as_bytes();
    super::progress::set_stage("Deriving key");
    let argon_out = Zeroizing::new(argon2id(password, &argon_salt, params)?);
    let mut ikm = Zeroizing::new([0u8; 64]);
    ikm[..32].copy_from_slice(seed.as_slice());
    ikm[32..].copy_from_slice(argon_out.as_slice());
    let secret = Zeroizing::new(hkdf(ikm.as_slice(), Some(&argon_salt), MASTER_INFO));
    let sign_seed = Zeroizing::new(hkdf(secret.as_slice(), None, SIGN_INFO));
    let wrap_seed = Zeroizing::new(hkdf(secret.as_slice(), None, WRAP_INFO));
    let sign = SigningKey::from_bytes(&sign_seed);
    let sign_pub = sign.verifying_key().to_bytes();
    let wrap = StaticSecret::from(*wrap_seed);
    let wrap_pub = PublicKey::from(&wrap).to_bytes();
    let acp_id = id_from_sign_pub(&sign_pub);
    Ok(Master {
        secret,
        sign_seed,
        wrap_seed,
        acp_id,
        sign_pub,
        wrap_pub,
    })
}

/// Recover a 256-bit seed from a 24-word phrase or from three to five SLIP-39 shares.
/// The share passphrase is empty. Two shares are refused.
pub fn seed_from_material(text: &str) -> Result<Zeroizing<[u8; 32]>> {
    let found = super::slip39::extract_mnemonics(text);
    if found.len() >= 3 {
        let refs: Vec<&str> = found.iter().map(String::as_str).collect();
        return seed_from_shares(&refs);
    }
    if !found.is_empty() {
        bail!(
            "need 3 shares of one set, found {}. Two shares are not enough.",
            found.len()
        );
    }
    recover_seed(text)
}

fn seed_from_shares(mnemonics: &[&str]) -> Result<Zeroizing<[u8; 32]>> {
    if !(3..=5).contains(&mnemonics.len()) {
        bail!("need 3, 4 or 5 shares, got {}", mnemonics.len());
    }
    let mut expected: Option<Zeroizing<Vec<u8>>> = None;
    let n = mnemonics.len();
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                let got = super::slip39::combine(&[mnemonics[a], mnemonics[b], mnemonics[c]], "")?;
                if let Some(prev) = &expected {
                    if prev.as_slice() != got.as_slice() {
                        bail!("these shares do not all recover the same seed");
                    }
                } else {
                    expected = Some(got);
                }
            }
        }
    }
    let secret = expected.ok_or_else(|| anyhow::anyhow!("need 3 shares"))?;
    if secret.len() != 32 {
        bail!("shares did not recover a 256-bit seed");
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    seed.copy_from_slice(&secret);
    Ok(seed)
}

/// A new phrase from the operating system's random source.
pub fn new_entropy() -> [u8; 32] {
    let mut entropy = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut entropy);
    entropy
}

#[cfg(test)]
mod secret_not_leaked {
    use super::*;

    #[test]
    fn debug_redacts_the_master_secret() {
        let phrase = phrase_from_entropy(&[0x42; 32]).unwrap();
        let m = derive_master(&phrase, "correct horse battery staple", Argon2Params::FAST).unwrap();
        let shown = format!("{m:?}");
        assert!(shown.contains("[redacted]"), "{shown}");
        assert!(!shown.contains(&hex(&*m.secret)), "{shown}");
        assert!(!shown.contains(&phrase), "{shown}");
    }

    fn hex(b: &[u8]) -> String {
        data_encoding::HEXLOWER.encode(b)
    }
}
