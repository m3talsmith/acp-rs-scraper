//! The node keystore and the public record a screen can show without it.
//!
//! The keystore is XChaCha20-Poly1305 under a key derived from the unlocked
//! database key. It is mode `0600` and backups skip it. `identity.key` is not
//! written. The public record holds the label, the peer id, the fingerprint,
//! and the certificates.

use anyhow::{Context, Result, bail};
use std::path::Path;
use zeroize::Zeroizing;

use super::master;
use super::pin::Pin;
use acp_red_star_node_core::node::{
    self, NodeSecret, SignedCert, certify_first_node, decode_secret, encode_secret, keystore_key,
    sanitize_label, verification_words,
};
use acp_red_star_node_core::store::{open_blob, seal_blob};
use crate::config::{DB_REL_PATH, NODE_KEYSTORE_REL_PATH, NODE_PUBLIC_REL_PATH, NODE_SECRET_REL_PATH};

const MAGIC: &[u8] = b"ACPKEY01";
const AAD: &[u8] = b"acp-node-keystore-v1";

/// What `--node-id` and the header can show. No seeds.
#[derive(Clone, Debug)]
pub struct PublicNode {
    /// Name shown in the header.
    pub label: String,
    /// Pinned account id.
    pub acp_id: String,
    /// libp2p peer id.
    pub peer_id: String,
    /// Short fingerprint.
    pub fingerprint: String,
    /// Node ed25519 public key.
    pub sign_pub: [u8; 32],
    /// Node X25519 public key.
    pub enc_pub: [u8; 32],
    /// Nonce of the enrollment request, so the verification words can be rebuilt.
    pub nonce: [u8; 16],
    /// Unix time the certificates were issued.
    pub issued_at: u64,
    /// Unix time the node certificate expires.
    pub node_expires: u64,
    /// Unix time the authorizer certificate expires.
    pub authorizer_expires: u64,
    /// Authorizer certificate.
    pub authorizer: SignedCert,
    /// Node certificate.
    pub node: SignedCert,
}

impl PublicNode {
    /// Header text. Empty when this home has no node.
    pub fn chip(&self) -> String {
        format!("◉ {}", self.label)
    }

    /// Six verification words for this node's enrollment request.
    pub fn verification_words(&self) -> Result<String> {
        let request = node::enrollment_request(&self.sign_pub, &self.enc_pub, &self.label, &self.nonce);
        Ok(verification_words(&request)?.join(" "))
    }

    /// Read the public record, if `--node-init` has been run.
    pub fn load(home: &Path) -> Result<Option<Self>> {
        let path = home.join(NODE_PUBLIC_REL_PATH);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file: PublicFile = toml::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(Self {
            label: file.label,
            acp_id: file.acp_id,
            peer_id: file.peer_id,
            fingerprint: file.fingerprint,
            sign_pub: decode_key(&file.sign_pub)?,
            enc_pub: decode_key(&file.enc_pub)?,
            nonce: decode_nonce(&file.nonce)?,
            issued_at: file.issued_at,
            node_expires: file.node_expires,
            authorizer_expires: file.authorizer_expires,
            authorizer: SignedCert {
                signed: decode_hex(&file.authorizer_signed)?,
                sig: decode_sig(&file.authorizer_sig)?,
            },
            node: SignedCert {
                signed: decode_hex(&file.node_signed)?,
                sig: decode_sig(&file.node_sig)?,
            },
        }))
    }
}

/// Create the first node for the pinned account. Refuses to replace a keystore.
///
/// `entropy` is the 256-bit phrase seed, already recovered from the seal or
/// from the words the user typed. The master is derived, signs the two
/// certificates, and is dropped before this function returns.
pub fn create_first_node(
    home: &Path,
    password: &str,
    label: &str,
    entropy: &[u8; 32],
) -> Result<PublicNode> {
    let label = sanitize_label(label)?;
    let pin = Pin::load(home)?.ok_or_else(|| {
        anyhow::anyhow!("no acp-id is pinned yet. Run --identity-init on a terminal.")
    })?;
    let store_path = home.join(NODE_KEYSTORE_REL_PATH);
    if store_path.exists() {
        bail!(
            "a node keystore already exists at {}. Refusing to replace it.",
            store_path.display()
        );
    }
    let db = home.join(DB_REL_PATH);
    if !super::atrest::bootstrap_path(&db).is_file() && !db.is_file() {
        bail!(
            "this home has no encrypted database yet. Open acp-red-star once so the database exists, then run --node-init."
        );
    }
    super::atrest::prepare(&db, Some(password))?;
    let local = super::atrest::cached_key(&db).ok_or_else(|| {
        anyhow::anyhow!("the database did not unlock, so the node keystore was not written")
    })?;
    let phrase = Zeroizing::new(master::phrase_from_entropy(entropy)?);
    let master = master::derive_master(phrase.as_str(), password, pin.argon2)?;
    master.require_id(&pin.acp_id)?;
    let secret = NodeSecret::generate();
    let now = now_secs();
    let (authorizer, node_cert) = certify_first_node(&master, &secret, &label, now)?;
    drop(master);
    let mut nonce = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    write_keystore(&store_path, &local, &secret)?;
    if home.join(NODE_SECRET_REL_PATH).exists() {
        bail!("refusing to leave a plaintext node key on disk");
    }
    let public = PublicNode {
        label,
        acp_id: pin.acp_id.to_string(),
        peer_id: secret.peer_id(),
        fingerprint: secret.fingerprint(),
        sign_pub: secret.sign_pub,
        enc_pub: secret.enc_pub,
        nonce,
        issued_at: node_cert.uint("issued_at")?,
        node_expires: node_cert.uint("expires_at")?,
        authorizer_expires: authorizer.uint("expires_at")?,
        authorizer,
        node: node_cert,
    };
    public.store(home)?;
    Ok(public)
}

/// Open the keystore with the unlocked database key. A wrong key is an error.
pub fn open_keystore(home: &Path, local_key: &[u8; 32]) -> Result<NodeSecret> {
    let path = home.join(NODE_KEYSTORE_REL_PATH);
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let Some(body) = bytes.strip_prefix(MAGIC) else {
        bail!("the node keystore is not a recognised file");
    };
    let plain = open_blob(&keystore_key(local_key), AAD, body)
        .context("the database key does not open the node keystore")?;
    decode_secret(&plain)
}

impl PublicNode {
    fn store(&self, home: &Path) -> Result<()> {
        let path = home.join(NODE_PUBLIC_REL_PATH);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let file = PublicFile {
            label: self.label.clone(),
            acp_id: self.acp_id.clone(),
            peer_id: self.peer_id.clone(),
            fingerprint: self.fingerprint.clone(),
            sign_pub: data_encoding::HEXLOWER.encode(&self.sign_pub),
            enc_pub: data_encoding::HEXLOWER.encode(&self.enc_pub),
            nonce: data_encoding::HEXLOWER.encode(&self.nonce),
            authorizer: true,
            issued_at: self.issued_at,
            node_expires: self.node_expires,
            authorizer_expires: self.authorizer_expires,
            authorizer_signed: data_encoding::HEXLOWER.encode(&self.authorizer.signed),
            authorizer_sig: data_encoding::HEXLOWER.encode(&self.authorizer.sig),
            node_signed: data_encoding::HEXLOWER.encode(&self.node.signed),
            node_sig: data_encoding::HEXLOWER.encode(&self.node.sig),
        };
        write_private(&path, toml::to_string(&file)?.as_bytes())?;
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PublicFile {
    label: String,
    acp_id: String,
    peer_id: String,
    fingerprint: String,
    sign_pub: String,
    enc_pub: String,
    nonce: String,
    authorizer: bool,
    issued_at: u64,
    node_expires: u64,
    authorizer_expires: u64,
    authorizer_signed: String,
    authorizer_sig: String,
    node_signed: String,
    node_sig: String,
}

fn write_keystore(path: &Path, local_key: &[u8; 32], secret: &NodeSecret) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    write_private(path, &keystore_bytes(local_key, secret)?)?;
    Ok(())
}

/// The keystore sealed again under the same database key.
/// `None` when this home has no keystore. The node secret does not change.
pub(crate) fn resealed_keystore(home: &Path, local_key: &[u8; 32]) -> Result<Option<Vec<u8>>> {
    let path = home.join(NODE_KEYSTORE_REL_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let secret = open_keystore(home, local_key)?;
    Ok(Some(keystore_bytes(local_key, &secret)?))
}

fn keystore_bytes(local_key: &[u8; 32], secret: &NodeSecret) -> Result<Vec<u8>> {
    let plain = encode_secret(secret)?;
    let sealed = seal_blob(&keystore_key(local_key), AAD, &plain)?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + sealed.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&sealed);
    Ok(bytes)
}

fn write_private(path: &Path, body: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(body)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, body)?;
    }
    Ok(())
}

fn decode_key(hex: &str) -> Result<[u8; 32]> {
    let bytes = decode_hex(hex)?;
    let a: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .context("public key must be 32 bytes")?;
    Ok(a)
}

fn decode_nonce(hex: &str) -> Result<[u8; 16]> {
    let bytes = decode_hex(hex)?;
    let a: [u8; 16] = bytes
        .as_slice()
        .try_into()
        .context("nonce must be 16 bytes")?;
    Ok(a)
}

fn decode_sig(hex: &str) -> Result<[u8; 64]> {
    let bytes = decode_hex(hex)?;
    let a: [u8; 64] = bytes
        .as_slice()
        .try_into()
        .context("signature must be 64 bytes")?;
    Ok(a)
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    data_encoding::HEXLOWER
        .decode(hex.trim().as_bytes())
        .context("hex")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A hostname with letters, digits, and hyphens only, or `node`.
pub fn default_label() -> String {
    let raw = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "node".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::config::ACP_ID_REL_PATH;
    use acp_red_star_node_core::node::verify_chain;
    use acp_red_star_node_core::password::Argon2Params;

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "acp-node-{}-{}-{}",
            label,
            std::process::id(),
            now_secs()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_node_is_sealed_shown_and_not_cloned_by_a_second_call() {
        let home = scratch("init");
        let entropy = [0x42u8; 32];
        let phrase = master::phrase_from_entropy(&entropy).unwrap();
        let master = master::derive_master(&phrase, "correct horse battery staple", Argon2Params::FAST).unwrap();
        Pin {
            acp_id: master.acp_id.clone(),
            argon2: Argon2Params::FAST,
            sign_pub: master.sign_pub,
            wrap_pub: master.wrap_pub,
        }
        .store(&home)
        .unwrap();
        let db = home.join(DB_REL_PATH);
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        super::super::atrest::prepare(&db, Some("correct horse battery staple")).unwrap();
        let local = super::super::atrest::cached_key(&db).unwrap();
        let public = create_first_node(&home, "correct horse battery staple", "laptop", &entropy).unwrap();
        assert_eq!(public.label, "laptop");
        assert_eq!(public.acp_id, master.acp_id.as_str());
        assert!(!home.join(NODE_SECRET_REL_PATH).exists());
        assert!(home.join(ACP_ID_REL_PATH).is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(home.join(NODE_KEYSTORE_REL_PATH))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        let opened = open_keystore(&home, &local).unwrap();
        assert_eq!(opened.sign_pub, public.sign_pub);
        assert_ne!(opened.sign_pub, master.sign_pub);
        assert!(open_keystore(&home, &[0; 32]).is_err());
        verify_chain(&public.node, Some(&public.authorizer), &master.sign_pub, public.issued_at, Some("sync")).unwrap();
        let again = create_first_node(&home, "correct horse battery staple", "laptop", &entropy);
        assert!(again.unwrap_err().to_string().contains("already exists"));
        let shown = format!("{opened:?}");
        assert!(shown.contains("[redacted]"));
        assert!(!shown.contains(&data_encoding::HEXLOWER.encode(opened.sign_seed())));
        let _ = std::fs::remove_dir_all(&home);
    }
}
