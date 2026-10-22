//! Gated registration and login. The browser holds a cookie id, not a key.
//!
//! A registration code lasts 10 minutes and is bound to the browser session that
//! created it. Open registration activates the member. Closed leaves them pending.
//! Login is an Ed25519 signature over a nonce, not a second Diffie-Hellman exchange.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use acp_red_star_node_core::node::NodeSecret;

use rusqlite::OptionalExtension;

use super::access::{self, Member, MemberStatus, pub_hex};
use super::members::{self, InviteFile};
use super::{check_slug, get_org, now};
use crate::prelude::*;

/// Registration and login codes last this long. Open question 11.
pub const CODE_TTL_SECS: i64 = 10 * 60;
/// A session cookie lasts this long. Open question 11.
pub const SESSION_TTL_SECS: i64 = 8 * 60 * 60;
/// Attempts one browser session may make in a minute. Open question 14.
pub const SESSION_LIMIT: i64 = 10;
/// Attempts one organization may take in a minute. Open question 14.
pub const ORG_LIMIT: i64 = 30;

/// A code the browser shows and the app opens.
#[derive(Clone, Debug)]
pub struct Offer {
    /// Single-use code.
    pub code: String,
    /// CSRF token bound to the browser session. Not a key.
    pub csrf: String,
    /// Cookie value. An id, not a key.
    pub browser_session: String,
    /// Organization display name.
    pub name: String,
    /// Service public key fingerprint. Empty for login.
    pub fingerprint: String,
    /// Register invite, when this is a registration.
    pub invite: Option<InviteFile>,
    /// Login nonce, when this is a login.
    pub nonce: String,
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn fresh(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut buf);
    data_encoding::HEXLOWER.encode(&buf)
}

fn service_path(db: &Path, slug: &str) -> PathBuf {
    db.parent().unwrap_or_else(|| Path::new(".")).join(format!("org-service-{slug}.key"))
}

/// The dedicated organization service identity. Created the first time it is asked for.
///
/// Both seeds are sealed. This is not the owner's personal key (open question 13).
pub fn service_identity(db: &Path, slug: &str) -> Result<NodeSecret> {
    let path = service_path(db, slug);
    let key = crate::crypto::atrest::cached_key(db).ok_or_else(|| anyhow!("unlock the database before publishing an organization"))?;
    if path.is_file() {
        return access::load_identity(&path, &key);
    }
    let secret = NodeSecret::generate();
    access::store_identity(&path, &secret, &key)?;
    Ok(secret)
}

/// Seal a service identity with an explicit key. Tests that have no unlocked database use this.
pub fn store_service_identity(db: &Path, slug: &str, secret: &NodeSecret, local_key: &[u8; 32]) -> Result<()> {
    access::store_identity(&service_path(db, slug), secret, local_key)
}

/// Open a service identity sealed by [`store_service_identity`].
pub fn load_service_identity(db: &Path, slug: &str, local_key: &[u8; 32]) -> Result<NodeSecret> {
    access::load_identity(&service_path(db, slug), local_key)
}

fn rate_limit(c: &Connection, org_id: i64, browser: &str, kind: &str) -> Result<()> {
    let since = unix_now() - 60;
    let session: i64 = c.query_row(
        "SELECT COUNT(*) FROM org_access_requests WHERE org_id = ?1 AND browser_session = ?2 AND kind = ?3 AND created_at >= ?4",
        params![org_id, browser, kind, since],
        |r| r.get(0),
    )?;
    if session >= SESSION_LIMIT {
        bail!("too many attempts for this browser session");
    }
    let org: i64 = c.query_row(
        "SELECT COUNT(*) FROM org_access_requests WHERE org_id = ?1 AND kind = ?2 AND created_at >= ?3",
        params![org_id, kind, since],
        |r| r.get(0),
    )?;
    if org >= ORG_LIMIT {
        bail!("too many attempts for this organization");
    }
    Ok(())
}

/// Start a registration. The code is bound to `browser` when that cookie already exists.
pub fn begin_register(c: &Connection, slug: &str, service: &NodeSecret, browser: Option<&str>) -> Result<Offer> {
    let slug = check_slug(slug)?;
    let org = get_org(c, &slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if org.visibility != super::Visibility::Public {
        bail!("registration is unavailable");
    }
    if !org.published {
        bail!("registration is unavailable");
    }
    let browser = browser.unwrap_or(&fresh(16)).to_string();
    rate_limit(c, org.id, &browser, "register")?;
    let role = if org.default_register_role.eq_ignore_ascii_case("Owner") { "Viewer" } else { org.default_register_role.as_str() };
    let mut invite = InviteFile {
        id: fresh(16),
        slug: slug.clone(),
        role: role.to_string(),
        expires_at: unix_now() + CODE_TTL_SECS,
        inviter_ed25519: pub_hex(service),
        inviter_x25519: data_encoding::HEXLOWER.encode(&service.enc_pub),
        inviter_sig: String::new(),
        invitee_ed25519: None,
        invitee_x25519: None,
        invitee_sig: None,
    };
    invite.inviter_sig = data_encoding::HEXLOWER.encode(&service.sign(&members::invite_message(&invite)));
    let csrf = fresh(16);
    let payload = members::to_json(&invite)?;
    c.execute(
        "INSERT INTO org_access_requests (id, org_id, kind, browser_session, code, nonce, expires_at, used_at, identity_pub, csrf_token, fingerprint, payload, created_at)
         VALUES (?1, ?2, 'register', ?3, ?4, ?4, ?5, NULL, NULL, ?6, '', ?7, ?8)",
        params![invite.id, org.id, browser, invite.id, invite.expires_at, csrf, payload, unix_now()],
    )?;
    Ok(Offer {
        code: invite.id.clone(),
        csrf,
        browser_session: browser,
        name: org.name,
        fingerprint: service.fingerprint(),
        invite: Some(invite),
        nonce: String::new(),
    })
}

/// Finish a registration. A mismatch, a replay, a different browser, or a bad CSRF adds nobody.
pub fn complete_register(c: &Connection, service: &NodeSecret, answered: &InviteFile, typed: &str, browser: &str, csrf: &str) -> Result<Member> {
    let ex = members::verify_answer(answered, service, typed)?;
    let invitee = answered.invitee_ed25519.as_deref().ok_or_else(|| anyhow!("invite has no reply"))?;
    let row = request_row(c, &answered.id)?;
    if row.browser != browser {
        bail!("browser session does not match");
    }
    if row.csrf != csrf {
        bail!("csrf token does not match");
    }
    if row.used.is_some() {
        bail!("code already used");
    }
    if row.expires_at < unix_now() {
        bail!("code expired");
    }
    let member = access::register_member(c, row.org_id, invitee)?;
    c.execute(
        "UPDATE org_access_requests SET used_at = ?1, identity_pub = ?2, fingerprint = ?3 WHERE code = ?4",
        params![now(), invitee, ex.fingerprint, answered.id],
    )?;
    Ok(member)
}

/// Start a login. The nonce is what the app signs.
pub fn begin_login(c: &Connection, slug: &str, browser: Option<&str>) -> Result<Offer> {
    let slug = check_slug(slug)?;
    let org = get_org(c, &slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if !org.published {
        bail!("login is unavailable");
    }
    let browser = browser.unwrap_or(&fresh(16)).to_string();
    rate_limit(c, org.id, &browser, "login")?;
    let code = fresh(16);
    let nonce = fresh(16);
    let csrf = fresh(16);
    let expires = unix_now() + CODE_TTL_SECS;
    c.execute(
        "INSERT INTO org_access_requests (id, org_id, kind, browser_session, code, nonce, expires_at, used_at, identity_pub, csrf_token, fingerprint, created_at)
         VALUES (?1, ?2, 'login', ?3, ?1, ?4, ?5, NULL, NULL, ?6, '', ?7)",
        params![code, org.id, browser, nonce, expires, csrf, unix_now()],
    )?;
    Ok(Offer { code, csrf, browser_session: browser, name: org.name, fingerprint: String::new(), invite: None, nonce })
}

/// Canonical login bytes.
pub fn login_message(nonce: &str, slug: &str, code: &str) -> Vec<u8> {
    format!("acp-org-login-v1\n{nonce}\n{slug}\n{code}").into_bytes()
}

/// Approve a browser session when `signature` matches an active member.
pub fn complete_login(c: &Connection, slug: &str, code: &str, browser: &str, csrf: &str, identity: &str, signature: &[u8]) -> Result<String> {
    let row = request_row(c, code)?;
    if row.kind != "login" {
        bail!("not a login code");
    }
    if row.browser != browser || row.csrf != csrf {
        bail!("browser session does not match");
    }
    if row.used.is_some() {
        bail!("code already used");
    }
    if row.expires_at < unix_now() {
        bail!("code expired");
    }
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if org.id != row.org_id {
        bail!("code is for another organization");
    }
    let who = access::check_pub(identity)?;
    let member = access::members(c, org.id)?.into_iter().find(|m| m.identity_pub == who).ok_or_else(|| anyhow!("not a member"))?;
    if member.status != MemberStatus::Active {
        bail!("registration is pending");
    }
    access::verify(&who, &login_message(&row.nonce, slug, code), signature)?;
    let session = fresh(16);
    let expires = unix_now() + SESSION_TTL_SECS;
    c.execute(
        "INSERT INTO org_sessions (id, org_id, identity_pub, browser_session, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![session, org.id, who, browser, expires],
    )?;
    c.execute("UPDATE org_access_requests SET used_at = ?1, identity_pub = ?2 WHERE code = ?3", params![now(), who, code])?;
    Ok(session)
}

/// The member for a session cookie, or nothing when it is missing or expired.
pub fn session_member(c: &Connection, session: &str) -> Result<Option<Member>> {
    let found: Option<(i64, String, i64)> = c
        .query_row("SELECT org_id, identity_pub, expires_at FROM org_sessions WHERE id = ?1", params![session], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    let Some((org_id, identity, expires)) = found else { return Ok(None) };
    if expires < unix_now() {
        return Ok(None);
    }
    Ok(access::members(c, org_id)?.into_iter().find(|m| m.identity_pub == identity))
}

/// Force a code into the past. Tests use this for expiry.
pub fn expire_code(c: &Connection, code: &str) -> Result<()> {
    c.execute("UPDATE org_access_requests SET expires_at = ?1 WHERE code = ?2", params![unix_now() - 5, code])?;
    Ok(())
}

/// Force a session into the past.
pub fn expire_session(c: &Connection, session: &str) -> Result<()> {
    c.execute("UPDATE org_sessions SET expires_at = ?1 WHERE id = ?2", params![unix_now() - 5, session])?;
    Ok(())
}

/// What `--org-open-code` shows before it asks.
#[derive(Clone, Debug)]
pub struct CodeView {
    /// Organization slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// `register` or `login`.
    pub kind: String,
    /// Login nonce. Empty for a registration.
    pub nonce: String,
    /// Browser session the code is bound to.
    pub browser: String,
    /// CSRF token bound to that session.
    pub csrf: String,
    /// Register invite, when this is a registration.
    pub invite: Option<InviteFile>,
}

/// Load a code stored on this machine. The caller prints the name and fingerprint, then asks.
pub fn describe_code(c: &Connection, code: &str) -> Result<CodeView> {
    let row = request_row(c, code)?;
    let org = super::list_orgs(c)?.into_iter().find(|o| o.id == row.org_id).ok_or_else(|| anyhow!("no organization"))?;
    let payload: String = c.query_row("SELECT payload FROM org_access_requests WHERE code = ?1", params![code], |r| r.get(0))?;
    let invite = if row.kind == "register" && !payload.is_empty() { Some(members::from_json(&payload)?) } else { None };
    Ok(CodeView { slug: org.slug, name: org.name, kind: row.kind, nonce: row.nonce, browser: row.browser, csrf: row.csrf, invite })
}

/// Finish a login from a JSON body. The cookie supplies the browser session.
pub fn finish_login_body(c: &Connection, slug: &str, body: &str, browser: &str) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| anyhow!("login body is not json"))?;
    let code = v.get("code").and_then(|s| s.as_str()).ok_or_else(|| anyhow!("login body has no code"))?;
    let csrf = v.get("csrf").and_then(|s| s.as_str()).ok_or_else(|| anyhow!("login body has no csrf"))?;
    let identity = v.get("identity").and_then(|s| s.as_str()).ok_or_else(|| anyhow!("login body has no identity"))?;
    let sig_hex = v.get("signature").and_then(|s| s.as_str()).ok_or_else(|| anyhow!("login body has no signature"))?;
    let signature = data_encoding::HEXLOWER.decode(sig_hex.as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    complete_login(c, slug, code, browser, csrf, identity, &signature)
}

/// Pending registrations. Each line is the identity and the fingerprint the approver compares.
pub fn pending(c: &Connection, slug: &str) -> Result<Vec<(String, String)>> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let mut st = c.prepare(
        "SELECT m.identity_pub, COALESCE(r.fingerprint, '') FROM org_members m
         LEFT JOIN org_access_requests r ON r.identity_pub = m.identity_pub AND r.org_id = m.org_id AND r.kind = 'register'
         WHERE m.org_id = ?1 AND m.status = 'pending'",
    )?;
    let rows = st.query_map(params![org.id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// What a non-member would start or stop seeing.
pub fn gate_preview(gated_after: bool) -> &'static str {
    if gated_after {
        "A non-member would stop seeing public posts and public files. The landing page stays."
    } else {
        "A non-member would start seeing public posts and public files."
    }
}

struct RequestRow {
    org_id: i64,
    kind: String,
    browser: String,
    nonce: String,
    expires_at: i64,
    used: Option<String>,
    csrf: String,
}

fn request_row(c: &Connection, code: &str) -> Result<RequestRow> {
    c.query_row(
        "SELECT org_id, kind, browser_session, nonce, expires_at, used_at, csrf_token FROM org_access_requests WHERE code = ?1",
        params![code],
        |r| {
            Ok(RequestRow {
                org_id: r.get(0)?,
                kind: r.get(1)?,
                browser: r.get(2)?,
                nonce: r.get(3)?,
                expires_at: r.get(4)?,
                used: r.get(5)?,
                csrf: r.get(6)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("no code {code}"))
}

/// Cookie attributes. `Secure` is set only for HTTPS.
pub fn cookie_header(id: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("acp_org={id}; HttpOnly; SameSite=Strict{secure}")
}

/// Listen on loopback and answer registration and login until `shutdown`.
pub fn serve_until(db: &Path, slug: &str, addr: SocketAddr, ready: &std::sync::mpsc::Sender<SocketAddr>, shutdown: &AtomicBool) -> Result<()> {
    members::require_loopback(addr)?;
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let bound = listener.local_addr()?;
    let _ = ready.send(bound);
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => { let _ = answer(stream, db, slug); }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn answer(mut stream: TcpStream, db: &Path, slug: &str) -> Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let text = String::from_utf8_lossy(&buf[..n]);
    let mut lines = text.lines();
    let request = lines.next().unwrap_or("");
    let mut cookie = String::new();
    let mut csrf_header = String::new();
    for line in lines {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("cookie:") {
            cookie = rest.trim().to_string();
        }
        if let Some(rest) = line.strip_prefix("X-Acp-Csrf:") {
            csrf_header = rest.trim().to_string();
        }
        if line.is_empty() {
            break;
        }
    }
    let browser = cookie.split(';').find_map(|p| p.trim().strip_prefix("acp_org=").map(str::to_string)).unwrap_or_default();
    let c = super::org_conn(db)?;
    let (status, body, set_cookie) = if request.starts_with("GET /v1/register") {
        let service = service_identity(db, slug)?;
        match begin_register(&c, slug, &service, if browser.is_empty() { None } else { Some(browser.as_str()) }) {
            Ok(offer) => {
                let body = serde_json::json!({
                    "code": offer.code,
                    "csrf": offer.csrf,
                    "name": offer.name,
                    "fingerprint": offer.fingerprint,
                    "link": format!("redstar://org-register?code={}&origin=http://127.0.0.1", offer.code),
                })
                .to_string();
                ("200 OK", body, Some(cookie_header(&offer.browser_session, false)))
            }
            Err(e) => ("403 Forbidden", e.to_string(), None),
        }
    } else if request.starts_with("GET /v1/poll") {
        let code = request.split("code=").nth(1).unwrap_or("").split(' ').next().unwrap_or("");
        match poll(&c, code, &browser) {
            Ok(body) => ("200 OK", body, None),
            Err(e) => ("403 Forbidden", e.to_string(), None),
        }
    } else if request.starts_with("POST /v1/login/finish") {
        let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
        match finish_login_body(&c, slug, body, &browser) {
            Ok(session) => ("200 OK", serde_json::json!({"session": session}).to_string(), None),
            Err(e) => ("403 Forbidden", e.to_string(), None),
        }
    } else if request.starts_with("POST /v1/login") {
        match begin_login(&c, slug, if browser.is_empty() { None } else { Some(browser.as_str()) }) {
            Ok(offer) => (
                "200 OK",
                serde_json::json!({"code": offer.code, "nonce": offer.nonce, "csrf": offer.csrf, "name": offer.name, "slug": slug}).to_string(),
                Some(cookie_header(&offer.browser_session, false)),
            ),
            Err(e) => ("403 Forbidden", e.to_string(), None),
        }
    } else {
        let _ = csrf_header;
        ("404 Not Found", "not found".into(), None)
    };
    let cookie_line = set_cookie.map(|v| format!("Set-Cookie: {v}\r\n")).unwrap_or_default();
    let raw = format!("HTTP/1.0 {status}\r\nContent-Type: application/json\r\n{cookie_line}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(raw.as_bytes())?;
    Ok(())
}

fn poll(c: &Connection, code: &str, browser: &str) -> Result<String> {
    let row = request_row(c, code)?;
    if row.browser != browser {
        bail!("browser session does not match");
    }
    let status = if row.used.is_some() { "registered" } else { "pending" };
    Ok(serde_json::json!({"status": status}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orgs::{OrgEdit, Visibility, create_org, org_conn, update_org};

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "acp-org-gate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn setup() -> (Connection, PathBuf, NodeSecret) {
        let d = dir();
        let db = d.join("db.sqlite");
        let c = org_conn(&db).unwrap();
        let owner = NodeSecret::generate();
        create_org(&c, "hold", "Rebel Hold", "secret-description", Visibility::Private, Visibility::Private, &pub_hex(&owner)).unwrap();
        update_org(&c, "hold", &OrgEdit { name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: Some(Visibility::Public) }).unwrap();
        c.execute("UPDATE organizations SET published = 1 WHERE slug = 'hold'", []).unwrap();
        let service = NodeSecret::generate();
        store_service_identity(&db, "hold", &service, &[3u8; 32]).unwrap();
        (c, db, service)
    }

    #[test]
    fn closed_registration_stays_pending_until_approval_and_a_bad_code_adds_nobody() {
        let (c, _db, service) = setup();
        let offer = begin_register(&c, "hold", &service, None).unwrap();
        assert!(offer.invite.as_ref().unwrap().inviter_sig.len() > 10);
        let app = NodeSecret::generate();
        let invite = offer.invite.unwrap();
        let fp = members::shown_fingerprint(&app, &invite).unwrap();
        assert!(complete_register(&c, &service, &invite, "00000000", &offer.browser_session, &offer.csrf).is_err());
        let (answered, _) = members::accept_invite(&invite, &app, &fp).unwrap();
        let err = complete_register(&c, &service, &answered, &fp, "other-browser", &offer.csrf).unwrap_err();
        assert!(err.to_string().contains("browser session"), "{err}");
        let err = complete_register(&c, &service, &answered, &fp, &offer.browser_session, "nope").unwrap_err();
        assert!(err.to_string().contains("csrf"), "{err}");
        let member = complete_register(&c, &service, &answered, &fp, &offer.browser_session, &offer.csrf).unwrap();
        assert_eq!(member.status, MemberStatus::Pending);
        assert_eq!(member.role, "Viewer");
        let err = complete_register(&c, &service, &answered, &fp, &offer.browser_session, &offer.csrf).unwrap_err();
        assert!(err.to_string().contains("already used"), "{err}");
        let owner = access::members(&c, 1).unwrap().into_iter().find(|m| m.role == "Owner").unwrap();
        let err = access::approve_registration(&c, 1, &pub_hex(&service), &member.identity_pub).unwrap_err();
        assert!(err.to_string().contains("not a member"), "{err}");
        access::approve_registration(&c, 1, &owner.identity_pub, &member.identity_pub).unwrap();
        assert_eq!(access::members(&c, 1).unwrap().iter().find(|m| m.identity_pub == member.identity_pub).unwrap().status, MemberStatus::Active);
        let file = members::to_json(&answered).unwrap();
        assert!(!file.contains("sign_seed"));
        assert!(cookie_header("abc", false).contains("HttpOnly"));
        assert!(cookie_header("abc", false).contains("SameSite=Strict"));
        assert!(!cookie_header("abc", false).contains("Secure"));
        assert!(cookie_header("abc", true).contains("Secure"));
    }

    #[test]
    fn open_registration_is_active_and_login_rejects_replay_expiry_and_the_wrong_key() {
        let (c, _db, service) = setup();
        c.execute("UPDATE organizations SET registration = 'open' WHERE slug = 'hold'", []).unwrap();
        let offer = begin_register(&c, "hold", &service, None).unwrap();
        let app = NodeSecret::generate();
        let invite = offer.invite.unwrap();
        let fp = members::shown_fingerprint(&app, &invite).unwrap();
        let (answered, _) = members::accept_invite(&invite, &app, &fp).unwrap();
        let member = complete_register(&c, &service, &answered, &fp, &offer.browser_session, &offer.csrf).unwrap();
        assert_eq!(member.status, MemberStatus::Active);
        assert_eq!(member.role, "Viewer");

        let login = begin_login(&c, "hold", None).unwrap();
        let err = complete_login(&c, "hold", &login.code, "other", &login.csrf, &pub_hex(&app), &app.sign(&login_message(&login.nonce, "hold", &login.code))).unwrap_err();
        assert!(err.to_string().contains("browser session"), "{err}");
        let wrong = NodeSecret::generate();
        let err = complete_login(&c, "hold", &login.code, &login.browser_session, &login.csrf, &pub_hex(&wrong), &wrong.sign(&login_message(&login.nonce, "hold", &login.code))).unwrap_err();
        assert!(err.to_string().contains("not a member"), "{err}");
        let bad = app.sign(b"not the nonce");
        let err = complete_login(&c, "hold", &login.code, &login.browser_session, &login.csrf, &pub_hex(&app), &bad).unwrap_err();
        assert!(err.to_string().contains("signature"), "{err}");
        let session = complete_login(&c, "hold", &login.code, &login.browser_session, &login.csrf, &pub_hex(&app), &app.sign(&login_message(&login.nonce, "hold", &login.code))).unwrap();
        let who = session_member(&c, &session).unwrap().unwrap();
        assert_eq!(who.role, "Viewer");
        assert!(access::require(&c, 1, &who.identity_pub, "post").is_err());
        assert!(access::require(&c, 1, &who.identity_pub, "view").is_ok());
        let err = complete_login(&c, "hold", &login.code, &login.browser_session, &login.csrf, &pub_hex(&app), &app.sign(&login_message(&login.nonce, "hold", &login.code))).unwrap_err();
        assert!(err.to_string().contains("already used"), "{err}");
        expire_session(&c, &session).unwrap();
        assert!(session_member(&c, &session).unwrap().is_none());
        let login = begin_login(&c, "hold", None).unwrap();
        expire_code(&c, &login.code).unwrap();
        let err = complete_login(&c, "hold", &login.code, &login.browser_session, &login.csrf, &pub_hex(&app), &app.sign(&login_message(&login.nonce, "hold", &login.code))).unwrap_err();
        assert!(err.to_string().contains("expired"), "{err}");
    }

    #[test]
    fn a_private_org_has_no_registration_and_the_tenth_session_attempt_is_the_last() {
        let d = dir();
        let db = d.join("db.sqlite");
        let c = org_conn(&db).unwrap();
        let owner = NodeSecret::generate();
        create_org(&c, "hold", "Hold", "", Visibility::Private, Visibility::Private, &pub_hex(&owner)).unwrap();
        let service = NodeSecret::generate();
        let err = begin_register(&c, "hold", &service, None).unwrap_err();
        assert!(err.to_string().contains("unavailable"), "{err}");
        update_org(&c, "hold", &OrgEdit { name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: None }).unwrap();
        c.execute("UPDATE organizations SET published = 0 WHERE slug = 'hold'", []).unwrap();
        let err = begin_register(&c, "hold", &service, None).unwrap_err();
        assert!(err.to_string().contains("unavailable"), "{err}");
        c.execute("UPDATE organizations SET published = 1 WHERE slug = 'hold'", []).unwrap();
        let browser = "browser-1";
        for _ in 0..SESSION_LIMIT {
            begin_register(&c, "hold", &service, Some(browser)).unwrap();
        }
        let err = begin_register(&c, "hold", &service, Some(browser)).unwrap_err();
        assert!(err.to_string().contains("browser session"), "{err}");
        begin_register(&c, "hold", &service, Some("browser-2")).unwrap();
    }

    #[test]
    fn the_register_page_sets_an_httponly_cookie_and_a_poll_from_another_session_fails() {
        let (c, db, service) = setup();
        let offer = begin_register(&c, "hold", &service, None).unwrap();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let db2 = db.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || serve_until(&db2, "hold", "127.0.0.1:0".parse().unwrap(), &tx, &flag));
        let bound = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let other = http(bound, "GET", &format!("/v1/poll?code={}", offer.code), &[]);
        assert!(other.contains("403") || other.contains("browser session"));
        let same = http(bound, "GET", &format!("/v1/poll?code={}", offer.code), &[("Cookie", &format!("acp_org={}", offer.browser_session))]);
        assert!(same.contains("pending"));
        assert!(!same.contains("sign_seed"));
        stop.store(true, Ordering::Relaxed);
        let _ = server.join();
    }

    #[test]
    fn login_over_loopback_returns_a_session_and_a_second_post_is_refused() {
        let (c, db, _service) = setup();
        c.execute("UPDATE organizations SET registration = 'open' WHERE slug = 'hold'", []).unwrap();
        let app = NodeSecret::generate();
        access::register_member(&c, 1, &pub_hex(&app)).unwrap();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let db2 = db.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || serve_until(&db2, "hold", "127.0.0.1:0".parse().unwrap(), &tx, &flag));
        let bound = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let began = http(bound, "POST", "/v1/login", &[]);
        assert!(began.contains("200"), "{began}");
        let code = json_field(&began, "code");
        let nonce = json_field(&began, "nonce");
        let csrf = json_field(&began, "csrf");
        let slug = json_field(&began, "slug");
        let cookie = began.split("Set-Cookie:").nth(1).unwrap().split(';').next().unwrap().trim();
        let sig = data_encoding::HEXLOWER.encode(&app.sign(&login_message(&nonce, &slug, &code)));
        let body = serde_json::json!({"code": code, "csrf": csrf, "identity": pub_hex(&app), "signature": sig}).to_string();
        let done = http_body(bound, "POST", "/v1/login/finish", &[("Cookie", cookie)], &body);
        assert!(done.contains("session"), "{done}");
        let again = http_body(bound, "POST", "/v1/login/finish", &[("Cookie", cookie)], &body);
        assert!(again.contains("already used") || again.contains("403"), "{again}");
        stop.store(true, Ordering::Relaxed);
        let _ = server.join();
    }

    fn json_field(raw: &str, key: &str) -> String {
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or(raw);
        let v: serde_json::Value = serde_json::from_str(body).unwrap();
        v.get(key).and_then(|s| s.as_str()).unwrap().to_string()
    }

    fn http(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
        let mut sock = TcpStream::connect(addr).unwrap();
        let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{extra}Connection: close\r\n\r\n");
        sock.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        sock.read_to_string(&mut out).unwrap();
        out
    }

    fn http_body(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut sock = TcpStream::connect(addr).unwrap();
        let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        sock.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        sock.read_to_string(&mut out).unwrap();
        out
    }
}
