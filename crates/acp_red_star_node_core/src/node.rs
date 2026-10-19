//! A node's own keys, its name, and the certificates that bind it to an account.
//!
//! The node key is generated on the machine. It is not derived from the master.
//! The master signs an authorizer certificate and the first node certificate,
//! then the caller drops the master. An authorizer certificate lasts 30 days,
//! a node certificate lasts 365 days, and clocks may disagree by 5 minutes
//! (decided 2026-10-19).

use anyhow::{Result, bail};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::cbor::{Cbor, from_slice, to_vec};
use super::master::Master;

/// Node-certificate lifetime. Decided 2026-10-19: 365 days.
pub const NODE_TTL_SECS: u64 = 365 * 24 * 60 * 60;
/// Authorizer-certificate lifetime. Decided 2026-10-19: 30 days.
pub const AUTHORIZER_TTL_SECS: u64 = 30 * 24 * 60 * 60;
/// How far a certificate time may be from this machine's clock. Decided 2026-10-19: 5 minutes.
pub const CLOCK_SKEW_SECS: u64 = 5 * 60;
/// How many nodes the first authorizer may certify. A local default.
pub const DEFAULT_MAX_NODES: u64 = 16;

const B58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// ed25519 signing key and X25519 wrapping key for one machine.
///
/// Debug output says `[redacted]` and does not include either seed.
pub struct NodeSecret {
    sign: Zeroizing<[u8; 32]>,
    enc: Zeroizing<[u8; 32]>,
    /// ed25519 public key. This is also the libp2p identity.
    pub sign_pub: [u8; 32],
    /// X25519 public key. Account data keys are wrapped to this.
    pub enc_pub: [u8; 32],
}

impl std::fmt::Debug for NodeSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSecret")
            .field("sign", &"[redacted]")
            .field("enc", &"[redacted]")
            .field("sign_pub", &hex(&self.sign_pub))
            .field("enc_pub", &hex(&self.enc_pub))
            .finish()
    }
}

impl NodeSecret {
    /// Draw a new keypair from the operating system's random source.
    pub fn generate() -> Self {
        let mut sign = [0u8; 32];
        let mut enc = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut sign);
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut enc);
        Self::from_seeds(sign, enc)
    }

    /// Rebuild a node from seeds that were sealed at rest. The arrays are zeroized
    /// by the caller; this value keeps its own copies.
    pub fn from_seeds(sign: [u8; 32], enc: [u8; 32]) -> Self {
        let sign_key = SigningKey::from_bytes(&sign);
        let sign_pub = sign_key.verifying_key().to_bytes();
        let enc_secret = x25519_dalek::StaticSecret::from(enc);
        let enc_pub = x25519_dalek::PublicKey::from(&enc_secret).to_bytes();
        Self {
            sign: Zeroizing::new(sign),
            enc: Zeroizing::new(enc),
            sign_pub,
            enc_pub,
        }
    }

    /// The signing seed. Callers that seal it must zeroize their copy.
    pub fn sign_seed(&self) -> &[u8; 32] {
        &self.sign
    }

    /// The wrapping seed. Callers that seal it must zeroize their copy.
    pub fn enc_seed(&self) -> &[u8; 32] {
        &self.enc
    }

    /// Sign `msg`. The seed is not included in the returned signature.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        SigningKey::from_bytes(&self.sign).sign(msg).to_bytes()
    }

    /// libp2p PeerId for this node's ed25519 key.
    pub fn peer_id(&self) -> String {
        peer_id_from_pubkey(&self.sign_pub)
    }

    /// Short id for a screen: 16 hex characters, the first 8 bytes of blake3(public key).
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.sign_pub)
    }
}

/// A certificate and the exact bytes the issuer signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedCert {
    /// Canonical CBOR of the certificate fields. The signature is not a field.
    pub signed: Vec<u8>,
    /// ed25519 signature over [`Self::signed`].
    pub sig: [u8; 64],
}

impl SignedCert {
    /// The certificate fields.
    pub fn body(&self) -> Result<Cbor> {
        from_slice(&self.signed)
    }

    /// An unsigned integer field.
    pub fn uint(&self, key: &str) -> Result<u64> {
        field_u64(&self.body()?, key)
    }

    /// A text field.
    pub fn text(&self, key: &str) -> Result<String> {
        match field(&self.body()?, key)? {
            Cbor::Text(t) => Ok(t.clone()),
            _ => bail!("certificate field {key} is not text"),
        }
    }
}

/// Reject an empty name, a control character, or a name longer than 64 characters.
pub fn sanitize_label(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("a node needs a name");
    }
    if trimmed.chars().count() > 64 {
        bail!("a node name is at most 64 characters");
    }
    if trimmed.chars().any(char::is_control) {
        bail!("a node name cannot contain control characters");
    }
    Ok(trimmed.to_string())
}

/// libp2p PeerId (base58btc) of an ed25519 public key.
///
/// The protobuf public key is `08 01 12 20` followed by the 32 key bytes.
/// The PeerId is the base58btc of a sha2-256 multihash of that protobuf.
pub fn peer_id_from_pubkey(sign_pub: &[u8; 32]) -> String {
    let mut proto = Vec::with_capacity(36);
    proto.extend_from_slice(&[0x08, 0x01, 0x12, 0x20]);
    proto.extend_from_slice(sign_pub);
    let dig = Sha256::digest(&proto);
    let mut multi = Vec::with_capacity(34);
    multi.push(0x12);
    multi.push(0x20);
    multi.extend_from_slice(&dig);
    base58btc(&multi)
}

/// 16 hex characters from blake3 of the node signing public key.
pub fn fingerprint_of(sign_pub: &[u8; 32]) -> String {
    let full = blake3::hash(sign_pub);
    hex(&full.as_bytes()[..8])
}

/// Canonical enrollment request. The first machine does not send it anywhere;
/// the verification words are how a later device is compared.
pub fn enrollment_request(
    sign_pub: &[u8; 32],
    enc_pub: &[u8; 32],
    label: &str,
    nonce: &[u8; 16],
) -> Cbor {
    Cbor::map(vec![
        (Cbor::text("enc"), Cbor::bytes(*enc_pub)),
        (Cbor::text("label"), Cbor::text(label)),
        (Cbor::text("nonce"), Cbor::bytes(*nonce)),
        (Cbor::text("sign"), Cbor::bytes(*sign_pub)),
        (Cbor::text("type"), Cbor::text("enroll-request")),
        (Cbor::text("v"), Cbor::uint(1)),
    ])
}

/// Six BIP-39 words from the first 66 bits of blake3(canonical request).
///
/// Word 0 is the top 11 bits. The same request always produces the same words.
pub fn verification_words(request: &Cbor) -> Result<[String; 6]> {
    let hash = blake3::hash(&to_vec(request)?);
    let mut bits: u128 = 0;
    for byte in &hash.as_bytes()[..9] {
        bits = (bits << 8) | u128::from(*byte);
    }
    bits >>= 6;
    let list = bip39::Language::English.word_list();
    let mut words = std::array::from_fn(|_| String::new());
    for (i, word) in words.iter_mut().enumerate() {
        let shift = (5 - i) * 11;
        let idx = ((bits >> shift) & 0x7ff) as usize;
        *word = list[idx].to_string();
    }
    Ok(words)
}

/// Master-signed authorizer certificate and node certificate for the first machine.
///
/// Serial 1 is the authorizer. Serial 2 is the node. The node certificate's
/// issuer is the master, so the chain is node → master. The authorizer
/// certificate lets this same key certify later nodes. `history_grant` is
/// `allowed` because this machine already holds the database; the default for
/// a later node is still an open question.
pub fn certify_first_node(
    master: &Master,
    node: &NodeSecret,
    label: &str,
    now: u64,
) -> Result<(SignedCert, SignedCert)> {
    if node.sign_pub == master.sign_pub {
        bail!("the node key must not be the master key");
    }
    let label = sanitize_label(label)?;
    let authorizer = sign_with_master(
        master,
        authorizer_body(&master.sign_pub, &node.sign_pub, 1, now, now + AUTHORIZER_TTL_SECS),
    )?;
    let node_cert = sign_with_master(
        master,
        node_body(
            &master.sign_pub,
            node,
            &label,
            &master.sign_pub,
            2,
            now,
            now + NODE_TTL_SECS,
        ),
    )?;
    Ok((authorizer, node_cert))
}

/// Check a node certificate (and its authorizer, when the issuer is not the master).
///
/// `now` is this machine's clock, in unix seconds. `cap` is required when set.
pub fn verify_chain(
    node: &SignedCert,
    authorizer: Option<&SignedCert>,
    master_pub: &[u8; 32],
    now: u64,
    cap: Option<&str>,
) -> Result<()> {
    let body = node.body()?;
    require_text(&body, "type", "node")?;
    require_u64(&body, "v", 1)?;
    let account = require_bytes32(&body, "account")?;
    if account != *master_pub {
        bail!("the node certificate is for a different account");
    }
    let issuer = require_bytes32(&body, "issuer")?;
    verify_sig(&issuer, &node.signed, &node.sig)?;
    let issued = field_u64(&body, "issued_at")?;
    let expires = field_u64(&body, "expires_at")?;
    check_window(issued, expires, now)?;
    if let Some(cap) = cap {
        let caps = field_texts(&body, "caps")?;
        if !caps.iter().any(|c| c == cap) {
            bail!("the node certificate does not allow {cap}");
        }
    }
    if issuer == *master_pub {
        return Ok(());
    }
    let Some(authorizer) = authorizer else {
        bail!("the node certificate is not signed by the pinned master");
    };
    let auth = authorizer.body()?;
    require_text(&auth, "type", "authorizer")?;
    require_u64(&auth, "v", 1)?;
    if require_bytes32(&auth, "account")? != *master_pub {
        bail!("the authorizer certificate is for a different account");
    }
    if require_bytes32(&auth, "subject")? != issuer {
        bail!("the authorizer did not issue this node certificate");
    }
    verify_sig(master_pub, &authorizer.signed, &authorizer.sig)?;
    let auth_issued = field_u64(&auth, "issued_at")?;
    let auth_expires = field_u64(&auth, "expires_at")?;
    check_window(auth_issued, auth_expires, now)?;
    if expires > auth_expires {
        bail!("the node certificate outlives its authorizer");
    }
    Ok(())
}

/// Seal the two seeds as canonical CBOR. The buffer is zeroized when dropped.
pub fn encode_secret(node: &NodeSecret) -> Result<Zeroizing<Vec<u8>>> {
    let value = Cbor::map(vec![
        (Cbor::text("enc"), Cbor::bytes(*node.enc_seed())),
        (Cbor::text("sign"), Cbor::bytes(*node.sign_seed())),
        (Cbor::text("v"), Cbor::uint(1)),
    ]);
    Ok(Zeroizing::new(to_vec(&value)?))
}

/// Open a blob from [`encode_secret`].
pub fn decode_secret(bytes: &[u8]) -> Result<NodeSecret> {
    let value = from_slice(bytes).map_err(|_| anyhow::anyhow!("the node keystore is damaged"))?;
    let sign = require_bytes32(&value, "sign")?;
    let enc = require_bytes32(&value, "enc")?;
    Ok(NodeSecret::from_seeds(sign, enc))
}

/// HKDF key that seals the node keystore. `local_key` is the database key
/// already unlocked for this process. This does not run Argon2id again.
pub fn keystore_key(local_key: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, local_key);
    let mut key = Zeroizing::new([0u8; 32]);
    hk.expand(b"acp-node-keystore-v1", key.as_mut())
        .expect("32-byte HKDF output is valid");
    key
}

fn authorizer_body(
    account: &[u8; 32],
    subject: &[u8; 32],
    serial: u64,
    issued_at: u64,
    expires_at: u64,
) -> Cbor {
    let limits = Cbor::map(vec![
        (Cbor::text("history_grant"), Cbor::text("allowed")),
        (Cbor::text("max_node_ttl"), Cbor::uint(NODE_TTL_SECS)),
        (Cbor::text("max_nodes"), Cbor::uint(DEFAULT_MAX_NODES)),
        (Cbor::text("may_revoke"), Cbor::text("own")),
    ]);
    Cbor::map(vec![
        (Cbor::text("account"), Cbor::bytes(*account)),
        (Cbor::text("expires_at"), Cbor::uint(expires_at)),
        (Cbor::text("issued_at"), Cbor::uint(issued_at)),
        (Cbor::text("limits"), limits),
        (Cbor::text("serial"), Cbor::uint(serial)),
        (Cbor::text("subject"), Cbor::bytes(*subject)),
        (Cbor::text("type"), Cbor::text("authorizer")),
        (Cbor::text("v"), Cbor::uint(1)),
    ])
}

fn node_body(
    account: &[u8; 32],
    node: &NodeSecret,
    label: &str,
    issuer: &[u8; 32],
    serial: u64,
    issued_at: u64,
    expires_at: u64,
) -> Cbor {
    let caps = Cbor::array(vec![
        Cbor::text("intent-sender"),
        Cbor::text("publish"),
        Cbor::text("sync"),
    ]);
    Cbor::map(vec![
        (Cbor::text("account"), Cbor::bytes(*account)),
        (Cbor::text("caps"), caps),
        (Cbor::text("expires_at"), Cbor::uint(expires_at)),
        (Cbor::text("issued_at"), Cbor::uint(issued_at)),
        (Cbor::text("issuer"), Cbor::bytes(*issuer)),
        (Cbor::text("label"), Cbor::text(label)),
        (Cbor::text("node_enc_pub"), Cbor::bytes(node.enc_pub)),
        (Cbor::text("node_sign_pub"), Cbor::bytes(node.sign_pub)),
        (Cbor::text("peer_id"), Cbor::text(node.peer_id())),
        (Cbor::text("serial"), Cbor::uint(serial)),
        (Cbor::text("type"), Cbor::text("node")),
        (Cbor::text("v"), Cbor::uint(1)),
    ])
}

fn sign_with_master(master: &Master, body: Cbor) -> Result<SignedCert> {
    let signed = to_vec(&body)?;
    let sig = master.sign(&signed);
    Ok(SignedCert { signed, sig })
}

fn verify_sig(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> Result<()> {
    let key = VerifyingKey::from_bytes(pk).map_err(|_| anyhow::anyhow!("bad signing key"))?;
    let signature = ed25519_dalek::Signature::from_bytes(sig);
    key.verify(msg, &signature)
        .map_err(|_| anyhow::anyhow!("certificate signature does not match"))
}

fn check_window(issued: u64, expires: u64, now: u64) -> Result<()> {
    if expires < issued {
        bail!("certificate expiry is before it was issued");
    }
    let not_before = issued.saturating_sub(CLOCK_SKEW_SECS);
    let not_after = expires.saturating_add(CLOCK_SKEW_SECS);
    if now < not_before {
        bail!("certificate is not valid yet");
    }
    if now > not_after {
        bail!("certificate has expired");
    }
    Ok(())
}

fn field<'a>(value: &'a Cbor, key: &str) -> Result<&'a Cbor> {
    let Cbor::Map(entries) = value else {
        bail!("certificate is not a map");
    };
    entries
        .iter()
        .find(|(k, _)| matches!(k, Cbor::Text(t) if t == key))
        .map(|(_, v)| v)
        .ok_or_else(|| anyhow::anyhow!("certificate is missing {key}"))
}

fn field_u64(value: &Cbor, key: &str) -> Result<u64> {
    match field(value, key)? {
        Cbor::Uint(n) => Ok(*n),
        _ => bail!("certificate field {key} is not an integer"),
    }
}

fn require_u64(value: &Cbor, key: &str, expect: u64) -> Result<()> {
    let got = field_u64(value, key)?;
    if got != expect {
        bail!("certificate field {key} is {got}");
    }
    Ok(())
}

fn require_text(value: &Cbor, key: &str, expect: &str) -> Result<()> {
    match field(value, key)? {
        Cbor::Text(t) if t == expect => Ok(()),
        _ => bail!("certificate field {key} is not {expect}"),
    }
}

fn require_bytes32(value: &Cbor, key: &str) -> Result<[u8; 32]> {
    match field(value, key)? {
        Cbor::Bytes(b) if b.len() == 32 => {
            let mut out = [0u8; 32];
            out.copy_from_slice(b);
            Ok(out)
        }
        _ => bail!("certificate field {key} is not 32 bytes"),
    }
}

fn field_texts(value: &Cbor, key: &str) -> Result<Vec<String>> {
    match field(value, key)? {
        Cbor::Array(items) => items
            .iter()
            .map(|item| match item {
                Cbor::Text(t) => Ok(t.clone()),
                _ => bail!("a capability is not text"),
            })
            .collect(),
        _ => bail!("certificate field {key} is not a list"),
    }
}

fn base58btc(data: &[u8]) -> String {
    let zeros = data.iter().take_while(|b| **b == 0).count();
    let mut digits: Vec<u8> = Vec::new();
    for &byte in data {
        let mut carry = u32::from(byte);
        for digit in &mut digits {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::new();
    out.extend(std::iter::repeat_n('1', zeros));
    for digit in digits.iter().rev() {
        out.push(B58[*digit as usize] as char);
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::{derive_master, phrase_from_entropy};
    use crate::password::Argon2Params;

    fn sample_master() -> Master {
        let phrase = phrase_from_entropy(&[0x42; 32]).unwrap();
        derive_master(&phrase, "correct horse battery staple", Argon2Params::FAST).unwrap()
    }

    #[test]
    fn node_key_is_not_the_master_and_debug_hides_seeds() {
        let master = sample_master();
        let node = NodeSecret::generate();
        assert_ne!(node.sign_pub, master.sign_pub);
        let shown = format!("{node:?}");
        assert!(shown.contains("[redacted]"), "{shown}");
        assert!(!shown.contains(&hex(node.sign_seed())), "{shown}");
        assert!(!shown.contains(&hex(node.enc_seed())), "{shown}");
        let err = certify_first_node(
            &master,
            &NodeSecret::from_seeds(*master.sign_seed_test(), [9; 32]),
            "home",
            1_700_000_000,
        )
        .unwrap_err();
        assert!(err.to_string().contains("must not be the master"), "{err}");
    }

    #[test]
    fn certificate_lifetimes_are_the_decided_ones() {
        assert_eq!(AUTHORIZER_TTL_SECS, 30 * 24 * 60 * 60);
        assert_eq!(NODE_TTL_SECS, 365 * 24 * 60 * 60);
        assert_eq!(CLOCK_SKEW_SECS, 5 * 60);
        assert!(AUTHORIZER_TTL_SECS < NODE_TTL_SECS);
        let master = sample_master();
        let node = NodeSecret::from_seeds([7; 32], [8; 32]);
        let now = 1_700_000_000u64;
        let (authorizer, cert) = certify_first_node(&master, &node, "home", now).unwrap();
        assert_eq!(authorizer.uint("expires_at").unwrap(), now + AUTHORIZER_TTL_SECS);
        assert_eq!(cert.uint("expires_at").unwrap(), now + NODE_TTL_SECS);
        assert_eq!(authorizer.uint("issued_at").unwrap(), now);
        assert_eq!(cert.uint("issued_at").unwrap(), now);
    }

    #[test]
    fn first_node_chain_checks_expiry_and_capability() {
        let master = sample_master();
        let node = NodeSecret::from_seeds([7; 32], [8; 32]);
        let now = 1_700_000_000u64;
        let (authorizer, cert) = certify_first_node(&master, &node, "  home  ", now).unwrap();
        verify_chain(&cert, Some(&authorizer), &master.sign_pub, now, Some("sync")).unwrap();
        verify_chain(&cert, None, &master.sign_pub, now, Some("publish")).unwrap();
        let err = verify_chain(&cert, None, &master.sign_pub, now, Some("relay")).unwrap_err();
        assert!(err.to_string().contains("does not allow"), "{err}");
        let later = now + NODE_TTL_SECS + CLOCK_SKEW_SECS + 1;
        assert!(verify_chain(&cert, None, &master.sign_pub, later, None).is_err());
        let mut bad = cert.clone();
        bad.sig[0] ^= 0x01;
        assert!(verify_chain(&bad, None, &master.sign_pub, now, None).is_err());
        let body = cert.body().unwrap();
        assert!(matches!(field(&body, "label"), Ok(Cbor::Text(t)) if t == "home"));
        assert!(matches!(field(&body, "serial"), Ok(Cbor::Uint(2))));
    }

    #[test]
    fn peer_id_and_words_are_stable() {
        let node = NodeSecret::from_seeds([0x11; 32], [0x22; 32]);
        assert_eq!(node.fingerprint(), "4a78877c4a0926fc");
        let id = node.peer_id();
        assert_eq!(id, "QmXS53cQyDRT7RaXiKYLjfkX8xSc9pBDPohDh1F3HxzjAz");
        let request = enrollment_request(&node.sign_pub, &node.enc_pub, "home", &[3; 16]);
        let words = verification_words(&request).unwrap();
        assert_eq!(
            words,
            ["zebra", "cloud", "van", "any", "butter", "science"]
        );
    }

    #[test]
    fn keystore_bytes_round_trip_with_a_derived_key() {
        let node = NodeSecret::from_seeds([4; 32], [5; 32]);
        let sealed = encode_secret(&node).unwrap();
        let opened = decode_secret(&sealed).unwrap();
        assert_eq!(opened.sign_pub, node.sign_pub);
        assert_eq!(opened.enc_pub, node.enc_pub);
        assert_eq!(opened.sign_seed(), node.sign_seed());
        let key = keystore_key(&[9; 32]);
        assert_ne!(key.as_slice(), &[9; 32]);
        assert_eq!(keystore_key(&[9; 32]).as_slice(), key.as_slice());
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(sanitize_label("  laptop ").unwrap(), "laptop");
        assert!(sanitize_label("   ").is_err());
        assert!(sanitize_label(&"n".repeat(65)).is_err());
        assert!(sanitize_label("bad\nname").is_err());
    }
}

