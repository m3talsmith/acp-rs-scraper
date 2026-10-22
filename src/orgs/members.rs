//! Invites and the membership fingerprint.
//!
//! Each side signs its X25519 public key with its Ed25519 identity. Both sides
//! run X25519 and HKDF-SHA256. The HKDF output is not shown. The person compares
//! a short fingerprint of the transcript and types it. A mismatch, an expired
//! invite, or a second finish adds nobody.
//!
//! The file and a `127.0.0.1` socket are the transports here. The same transcript
//! over QUIC is 3.49.0.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::time::{SystemTime, UNIX_EPOCH};

use acp_red_star_node_core::node::NodeSecret;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use rusqlite::OptionalExtension;

use super::access::{add_invited_member, check_pub, pub_hex};
use super::{check_slug, get_org, now};
use crate::prelude::*;

/// How long an invite stays usable. Open question 1 keeps 48 hours.
pub const INVITE_TTL_SECS: i64 = 48 * 60 * 60;
const HKDF_INFO: &[u8] = b"acp-org-invite-hkdf-v1";
/// Short code both screens show. Eight hex characters.
const FINGERPRINT_HEX: usize = 8;

/// An invite file. Public keys and signatures only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteFile {
    /// Single-use id.
    pub id: String,
    /// Organization slug.
    pub slug: String,
    /// Role the new member receives. Never Owner.
    pub role: String,
    /// Unix seconds.
    pub expires_at: i64,
    /// Inviter Ed25519 public key, hex.
    pub inviter_ed25519: String,
    /// Inviter X25519 public key, hex.
    pub inviter_x25519: String,
    /// Inviter signature over the invite fields.
    pub inviter_sig: String,
    /// Invitee Ed25519 public key, set when the invite is answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitee_ed25519: Option<String>,
    /// Invitee X25519 public key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitee_x25519: Option<String>,
    /// Invitee signature over the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitee_sig: Option<String>,
}

/// What both sides show, and the HKDF output they do not show.
#[derive(Clone)]
pub struct Exchange {
    /// Short fingerprint of the transcript.
    pub fingerprint: String,
    /// HKDF output. Not printed, not logged, not written.
    okm: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exchange").field("fingerprint", &self.fingerprint).field("okm", &"[redacted]").finish()
    }
}

impl Exchange {
    /// The HKDF output, for a test that checks both sides agree and that it is absent from a file.
    pub fn okm_for_test(&self) -> &[u8; 32] {
        &self.okm
    }
}

/// Canonical bytes the inviter signs.
pub fn invite_message(invite: &InviteFile) -> Vec<u8> {
    format!(
        "acp-org-invite-v1\n{}\n{}\n{}\n{}\n{}\n{}",
        invite.id, invite.slug, invite.role, invite.expires_at, invite.inviter_ed25519, invite.inviter_x25519
    )
    .into_bytes()
}

/// Canonical bytes the invitee signs.
pub fn accept_message(invite: &InviteFile, ed: &str, x: &str) -> Vec<u8> {
    format!("acp-org-invite-accept-v1\n{}\n{}\n{ed}\n{x}", invite.id, invite.slug).into_bytes()
}

fn transcript(invite: &InviteFile, invitee_ed: &str, invitee_x: &str) -> Vec<u8> {
    format!(
        "acp-org-invite-transcript-v1\n{}\n{}\n{}\n{invitee_ed}\n{invitee_x}",
        invite.id, invite.inviter_ed25519, invite.inviter_x25519
    )
    .into_bytes()
}

fn fingerprint_of(bytes: &[u8]) -> String {
    let dig = Sha256::digest(bytes);
    data_encoding::HEXLOWER.encode(&dig)[..FINGERPRINT_HEX].to_string()
}

fn shared_secret(mine: &NodeSecret, their_x: &str) -> Result<[u8; 32]> {
    let their = check_pub(their_x)?;
    let raw = data_encoding::HEXLOWER.decode(their.as_bytes()).map_err(|_| anyhow!("an identity key is 32 bytes of hex"))?;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&raw);
    let secret = x25519_dalek::StaticSecret::from(*mine.enc_seed());
    let public = x25519_dalek::PublicKey::from(arr);
    Ok(secret.diffie_hellman(&public).to_bytes())
}

fn exchange(mine: &NodeSecret, invite: &InviteFile, invitee_ed: &str, invitee_x: &str) -> Result<Exchange> {
    let shared = Zeroizing::new(shared_secret(mine, if pub_hex(mine) == invite.inviter_ed25519 { invitee_x } else { &invite.inviter_x25519 })?);
    let script = transcript(invite, invitee_ed, invitee_x);
    let hk = Hkdf::<Sha256>::new(Some(&script), shared.as_slice());
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm).map_err(|_| anyhow!("invite key derivation failed"))?;
    Ok(Exchange { fingerprint: fingerprint_of(&script), okm: Zeroizing::new(okm) })
}

fn verify_sig(public_hex: &str, message: &[u8], sig_hex: &str) -> Result<()> {
    let public_hex = check_pub(public_hex)?;
    let raw = data_encoding::HEXLOWER.decode(public_hex.as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    let mut key = [0u8; 32];
    if raw.len() != 32 {
        bail!("signature rejected");
    }
    key.copy_from_slice(&raw);
    let vk = VerifyingKey::from_bytes(&key).map_err(|_| anyhow!("signature rejected"))?;
    let sig_raw = data_encoding::HEXLOWER.decode(sig_hex.trim().as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    let sig = Signature::from_slice(&sig_raw).map_err(|_| anyhow!("signature rejected"))?;
    vk.verify(message, &sig).map_err(|_| anyhow!("signature rejected"))
}

fn sign_hex(secret: &NodeSecret, message: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(&secret.sign(message))
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn fresh_id() -> String {
    let mut buf = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut buf);
    data_encoding::HEXLOWER.encode(&buf)
}

/// Write an invite. The actor needs `invite`. Owner is refused as the new role.
pub fn create_invite(c: &Connection, slug: &str, inviter: &NodeSecret, role: &str, ttl_secs: i64) -> Result<InviteFile> {
    let slug = check_slug(slug)?;
    let org = get_org(c, &slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let actor = pub_hex(inviter);
    if role.eq_ignore_ascii_case("Owner") {
        bail!("the Owner role cannot be given to a new member");
    }
    let roles = super::access::roles(c, org.id)?;
    let found = roles.iter().find(|r| r.name.eq_ignore_ascii_case(role)).ok_or_else(|| anyhow!("no role {role:?}"))?;
    super::access::require(c, org.id, &actor, "invite")?;
    let mut invite = InviteFile {
        id: fresh_id(),
        slug,
        role: found.name.clone(),
        expires_at: unix_now().saturating_add(ttl_secs),
        inviter_ed25519: actor.clone(),
        inviter_x25519: data_encoding::HEXLOWER.encode(&inviter.enc_pub),
        inviter_sig: String::new(),
        invitee_ed25519: None,
        invitee_x25519: None,
        invitee_sig: None,
    };
    invite.inviter_sig = sign_hex(inviter, &invite_message(&invite));
    c.execute(
        "INSERT INTO org_invites (id, org_id, role_id, inviter_pub, expires_at, used_at, nonce) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
        params![invite.id, org.id, found.id, actor, invite.expires_at.to_string(), invite.id],
    )?;
    Ok(invite)
}

/// Check the inviter's signature and the expiry. A bad key or a past invite is refused.
pub fn open_invite(invite: &InviteFile) -> Result<()> {
    if invite.expires_at < unix_now() {
        bail!("invite expired");
    }
    verify_sig(&invite.inviter_ed25519, &invite_message(invite), &invite.inviter_sig)
}

/// Answer an invite. A fingerprint that does not match writes nothing.
pub fn accept_invite(invite: &InviteFile, invitee: &NodeSecret, typed: &str) -> Result<(InviteFile, Exchange)> {
    open_invite(invite)?;
    let ed = pub_hex(invitee);
    let x = data_encoding::HEXLOWER.encode(&invitee.enc_pub);
    let mut answered = invite.clone();
    answered.invitee_ed25519 = Some(ed.clone());
    answered.invitee_x25519 = Some(x.clone());
    answered.invitee_sig = Some(sign_hex(invitee, &accept_message(invite, &ed, &x)));
    let ex = exchange(invitee, invite, &ed, &x)?;
    if !code_matches(&ex.fingerprint, typed) {
        bail!("fingerprint does not match");
    }
    Ok((answered, ex))
}

/// Check the answer. Does not add a member and does not mark the invite used.
pub fn verify_answer(invite: &InviteFile, inviter: &NodeSecret, typed: &str) -> Result<Exchange> {
    open_invite(invite)?;
    let invitee_ed = invite.invitee_ed25519.as_deref().ok_or_else(|| anyhow!("invite has no reply"))?;
    let invitee_x = invite.invitee_x25519.as_deref().ok_or_else(|| anyhow!("invite has no reply"))?;
    let invitee_sig = invite.invitee_sig.as_deref().ok_or_else(|| anyhow!("invite has no reply"))?;
    verify_sig(invitee_ed, &accept_message(invite, invitee_ed, invitee_x), invitee_sig)?;
    if pub_hex(inviter) != invite.inviter_ed25519 {
        bail!("signature rejected");
    }
    let ex = exchange(inviter, invite, invitee_ed, invitee_x)?;
    if !code_matches(&ex.fingerprint, typed) {
        bail!("fingerprint does not match");
    }
    Ok(ex)
}

/// Finish an answered invite. The member is added only when the code matches.
pub fn finish_invite(c: &Connection, invite: &InviteFile, inviter: &NodeSecret, typed: &str) -> Result<Exchange> {
    let ex = verify_answer(invite, inviter, typed)?;
    let invitee_ed = invite.invitee_ed25519.as_deref().ok_or_else(|| anyhow!("invite has no reply"))?;
    let org = get_org(c, &invite.slug)?.ok_or_else(|| anyhow!("no organization {}", invite.slug))?;
    let row: Option<Option<String>> = c
        .query_row("SELECT used_at FROM org_invites WHERE id = ?1 AND org_id = ?2", params![invite.id, org.id], |r| r.get(0))
        .optional()?;
    let Some(used) = row else { bail!("invite is not from this organization") };
    if used.is_some() {
        bail!("invite already used");
    }
    add_invited_member(c, org.id, &invite.inviter_ed25519, invitee_ed, &invite.role)?;
    c.execute("UPDATE org_invites SET used_at = ?1 WHERE id = ?2", params![now(), invite.id])?;
    Ok(ex)
}

fn code_matches(expected: &str, typed: &str) -> bool {
    let typed = typed.trim().to_ascii_lowercase();
    !typed.is_empty() && typed == expected
}

/// The short code this identity would show for `invite`. The HKDF output is dropped.
pub fn shown_fingerprint(mine: &NodeSecret, invite: &InviteFile) -> Result<String> {
    let (ed, x) = if pub_hex(mine) == invite.inviter_ed25519 {
        let ed = invite.invitee_ed25519.clone().ok_or_else(|| anyhow!("invite has no reply"))?;
        let x = invite.invitee_x25519.clone().ok_or_else(|| anyhow!("invite has no reply"))?;
        (ed, x)
    } else {
        (pub_hex(mine), data_encoding::HEXLOWER.encode(&mine.enc_pub))
    };
    Ok(exchange(mine, invite, &ed, &x)?.fingerprint)
}

/// Serialize an invite. The JSON has public keys and signatures only.
pub fn to_json(invite: &InviteFile) -> Result<String> {
    Ok(serde_json::to_string_pretty(invite)?)
}

/// Read an invite file.
pub fn from_json(body: &str) -> Result<InviteFile> {
    Ok(serde_json::from_str(body)?)
}

/// Write the invite. Mode `0600`.
pub fn write_invite(path: &Path, invite: &InviteFile) -> Result<()> {
    super::access::write_private(path, to_json(invite)?.as_bytes())
}

/// Read an invite file from disk.
pub fn read_invite(path: &Path) -> Result<InviteFile> {
    let body = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    from_json(&body)
}

/// Refuse an address that is not IPv4 loopback, before any bind.
pub fn require_loopback(addr: SocketAddr) -> Result<()> {
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_loopback() => Ok(()),
        _ => bail!("organization invites listen on 127.0.0.1 only"),
    }
}

/// Bind `127.0.0.1` and return the listener plus the bound address.
pub fn listen_invite(addr: SocketAddr) -> Result<(TcpListener, SocketAddr)> {
    require_loopback(addr)?;
    let listener = TcpListener::bind(addr).with_context(|| format!("binding {addr}"))?;
    let bound = listener.local_addr()?;
    Ok((listener, bound))
}

/// Send one invite line and read one reply line.
pub fn send_invite(listener: TcpListener, invite_json: &str) -> Result<String> {
    let (mut sock, peer) = listener.accept()?;
    require_loopback(peer)?;
    sock.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    write_line(&mut sock, invite_json)?;
    read_line(&mut sock)
}

/// Read one invite line and send one reply line.
pub fn fetch_invite(addr: SocketAddr, reply_json: &str) -> Result<String> {
    require_loopback(addr)?;
    let mut sock = TcpStream::connect(addr).with_context(|| format!("connecting to {addr}"))?;
    sock.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    let invite = read_line(&mut sock)?;
    write_line(&mut sock, reply_json)?;
    Ok(invite)
}

fn write_line(sock: &mut TcpStream, body: &str) -> Result<()> {
    let one = body.replace('\n', "");
    sock.write_all(one.as_bytes())?;
    sock.write_all(b"\n")?;
    sock.flush()?;
    Ok(())
}

fn read_line(sock: &mut TcpStream) -> Result<String> {
    let mut line = String::new();
    BufReader::new(sock).read_line(&mut line)?;
    if line.is_empty() {
        bail!("invite connection closed");
    }
    Ok(line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::orgs::{Visibility, access, create_org, org_conn};

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "acp-org-members-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn setup() -> (Connection, NodeSecret, String) {
        let d = dir();
        let c = org_conn(&d.join("db.sqlite")).unwrap();
        let owner = NodeSecret::generate();
        let hex = pub_hex(&owner);
        create_org(&c, "hold", "Hold", "", Visibility::Private, Visibility::Private, &hex).unwrap();
        (c, owner, hex)
    }

    #[test]
    fn matching_fingerprint_adds_the_member_and_a_mismatch_does_not() {
        let (c, owner, owner_hex) = setup();
        let invite = create_invite(&c, "hold", &owner, "Viewer", INVITE_TTL_SECS).unwrap();
        let invitee = NodeSecret::generate();
        let wrong = accept_invite(&invite, &invitee, "00000000").unwrap_err();
        assert!(wrong.to_string().contains("fingerprint"), "{wrong}");
        assert_eq!(access::members(&c, 1).unwrap().len(), 1);
        let ed = pub_hex(&invitee);
        let x = data_encoding::HEXLOWER.encode(&invitee.enc_pub);
        let ex = exchange(&invitee, &invite, &ed, &x).unwrap();
        let (answered, accepted) = accept_invite(&invite, &invitee, &ex.fingerprint).unwrap();
        let finished = finish_invite(&c, &answered, &owner, &accepted.fingerprint).unwrap();
        assert_eq!(accepted.fingerprint, finished.fingerprint);
        assert_eq!(accepted.okm_for_test(), finished.okm_for_test());
        let people = access::members(&c, 1).unwrap();
        assert_eq!(people.len(), 2);
        assert!(people.iter().any(|m| m.identity_pub == ed && m.role == "Viewer" && m.status == access::MemberStatus::Active));
        let again = finish_invite(&c, &answered, &owner, &accepted.fingerprint).unwrap_err();
        assert!(again.to_string().contains("already used"), "{again}");
        assert_eq!(access::members(&c, 1).unwrap().len(), 2);
        let file = to_json(&answered).unwrap();
        let okm_hex = data_encoding::HEXLOWER.encode(accepted.okm_for_test());
        assert!(!file.contains(&okm_hex), "the invite file holds the HKDF output");
        assert!(!file.contains("sign_seed"));
        assert!(file.contains(&owner_hex));
        let shown = format!("{accepted:?} {finished:?}");
        assert!(shown.contains("[redacted]"));
        assert!(!shown.contains(&okm_hex));
    }

    #[test]
    fn expired_substituted_and_owner_role_are_refused() {
        let (c, owner, _) = setup();
        let mut invite = create_invite(&c, "hold", &owner, "Viewer", INVITE_TTL_SECS).unwrap();
        invite.expires_at = unix_now() - 5;
        let invitee = NodeSecret::generate();
        let err = accept_invite(&invite, &invitee, "abcdef01").unwrap_err();
        assert!(err.to_string().contains("expired"), "{err}");
        let err = create_invite(&c, "hold", &owner, "Owner", INVITE_TTL_SECS).unwrap_err();
        assert!(err.to_string().contains("Owner"), "{err}");

        let invite = create_invite(&c, "hold", &owner, "User", INVITE_TTL_SECS).unwrap();
        let mut tampered = invite.clone();
        tampered.inviter_x25519 = data_encoding::HEXLOWER.encode(&[9u8; 32]);
        let err = accept_invite(&tampered, &invitee, "abcdef01").unwrap_err();
        assert!(err.to_string().contains("signature rejected"), "{err}");
        assert_eq!(access::members(&c, 1).unwrap().len(), 1);
    }

    #[test]
    fn the_identity_file_is_mode_0600_and_the_seed_is_not_in_it() {
        let d = dir();
        let secret = NodeSecret::generate();
        let path = d.join("org-identity.key");
        access::store_identity(&path, &secret, &[7u8; 32]).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.windows(32).any(|w| w == secret.sign_seed().as_slice()));
        assert!(!bytes.windows(32).any(|w| w == secret.enc_seed().as_slice()));
        let loaded = access::load_identity(&path, &[7u8; 32]).unwrap();
        assert_eq!(loaded.sign_pub, secret.sign_pub);
        assert_eq!(format!("{secret:?}"), format!("{:?}", NodeSecret::from_seeds(*secret.sign_seed(), *secret.enc_seed())));
        assert!(format!("{secret:?}").contains("[redacted]"));
    }

    #[test]
    fn a_loopback_socket_carries_the_invite_and_other_addresses_do_not_bind() {
        let (c, owner, _) = setup();
        let invite = create_invite(&c, "hold", &owner, "Viewer", INVITE_TTL_SECS).unwrap();
        let invitee = NodeSecret::generate();
        let ed = pub_hex(&invitee);
        let x = data_encoding::HEXLOWER.encode(&invitee.enc_pub);
        let code = exchange(&invitee, &invite, &ed, &x).unwrap().fingerprint;
        let (answered, _) = accept_invite(&invite, &invitee, &code).unwrap();
        let line = serde_json::to_string(&invite).unwrap();
        let reply = serde_json::to_string(&answered).unwrap();
        let err = require_loopback("0.0.0.0:9".parse().unwrap()).unwrap_err();
        assert!(err.to_string().contains("127.0.0.1"), "{err}");
        let (listener, addr) = listen_invite("127.0.0.1:0".parse().unwrap()).unwrap();
        assert!(addr.ip().is_loopback());
        let server = std::thread::spawn(move || send_invite(listener, &line));
        let got = fetch_invite(addr, &reply).unwrap();
        let back = server.join().unwrap().unwrap();
        assert!(got.contains(&invite.id));
        assert!(back.contains(&ed));
        assert!(!back.contains("sign_seed"));
        let parsed = from_json(&back).unwrap();
        finish_invite(&c, &parsed, &owner, &code).unwrap();
        assert_eq!(access::members(&c, 1).unwrap().len(), 2);
    }
}
