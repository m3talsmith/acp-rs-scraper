//! An organization as a service on `127.0.0.1`.
//!
//! The adapter implements [`acp_rs_plugin::Service`], [`acp_rs_plugin::Reader`],
//! and [`acp_rs_plugin::Post`]. It is not registered at startup. Another install
//! adds the manifest after this one publishes. The peer transport is 3.49.0.
//! This listener is HTTP on loopback.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use acp_rs_plugin::{
    Availability, Budget, CapabilityId, CapFut, Cursor, Draft, ItemRec, Page, Resolved, RuntimeCtx,
    ServiceError, ServiceId, SourceId, WriteOutcome,
};

use super::access::{self, ANONYMOUS_AUTHOR};
use super::{Org, Visibility, get_org, list_posts, org_conn, public_row};
use crate::prelude::*;

const READER_ONLY: &[CapabilityId] = &[CapabilityId::Reader];
const READER_AND_POST: &[CapabilityId] = &[CapabilityId::Reader, CapabilityId::Post];

/// In-process adapter for one published organization.
pub struct OrgService {
    /// `org-<slug>`. Leaked so the plugin id can be `'static`.
    id: &'static str,
    name: &'static str,
    slug: String,
    db: PathBuf,
    /// Mirrors `organizations.allow_remote_post` at load.
    allow_posts: bool,
    /// A verified member. Anonymous reads stay false.
    caller_is_member: bool,
}

impl OrgService {
    /// Load the adapter from the database. Does not register it.
    pub fn load(db: &Path, slug: &str) -> Result<Self> {
        let c = org_conn(db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
        Ok(Self {
            id: Box::leak(format!("org-{slug}").into_boxed_str()),
            name: Box::leak(org.name.into_boxed_str()),
            slug: slug.to_string(),
            db: db.to_path_buf(),
            allow_posts: org.allow_remote_post,
            caller_is_member: false,
        })
    }
}

impl acp_rs_plugin::Service for OrgService {
    fn service_id(&self) -> ServiceId {
        ServiceId::new(self.id)
    }

    fn display_name(&self) -> &'static str {
        self.name
    }

    fn implemented(&self) -> &'static [CapabilityId] {
        if self.allow_posts { READER_AND_POST } else { READER_ONLY }
    }

    fn schema(&self) -> &'static [acp_rs_plugin::Field] {
        &[]
    }

    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
        if !ctx.enabled || !self.has(cap) {
            return Availability::Off;
        }
        let Ok(c) = org_conn(&self.db) else { return Availability::Off };
        let Ok(Some(org)) = get_org(&c, &self.slug) else { return Availability::Off };
        if org.visibility != Visibility::Public {
            return Availability::Off;
        }
        if cap == CapabilityId::Post && !org.allow_remote_post {
            return Availability::Off;
        }
        if org.gated && !ctx.logged_in {
            return Availability::Gated;
        }
        Availability::Ready
    }
}

impl acp_rs_plugin::Reader for OrgService {
    fn resolve<'a>(&'a self, target: &'a acp_rs_plugin::Target) -> CapFut<'a, Resolved> {
        let id = self.id;
        let slug = self.slug.clone();
        Box::pin(async move {
            Ok(Resolved {
                target: target.clone(),
                source_id: SourceId::new(ServiceId::new(id), slug),
            })
        })
    }

    fn page<'a>(&'a self, _resolved: &'a Resolved, _cursor: &'a Cursor, _stop_at: Option<&'a str>, _budget: &'a mut Budget) -> CapFut<'a, Page> {
        let member = self.caller_is_member;
        Box::pin(async move {
            let mut page = Page::empty();
            page.items = visible_items(&self.db, &self.slug, self.id, member).map_err(|e| ServiceError::Message(e.to_string()))?;
            Ok(page)
        })
    }
}

impl acp_rs_plugin::Post for OrgService {
    fn publish<'a>(&'a self, draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        Box::pin(async move {
            if !self.allow_posts {
                return Ok(WriteOutcome::NotReady(Availability::Off));
            }
            let c = org_conn(&self.db).map_err(|e| ServiceError::Message(e.to_string()))?;
            let org = get_org(&c, &self.slug).map_err(|e| ServiceError::Message(e.to_string()))?.ok_or_else(|| ServiceError::Message("no organization".into()))?;
            if !org.allow_remote_post {
                return Ok(WriteOutcome::NotReady(Availability::Off));
            }
            if org.gated || org.visibility != Visibility::Public {
                return Ok(WriteOutcome::NotReady(Availability::Gated));
            }
            let post = access::submit_guest_post(&c, &self.slug, "service", &draft.title, &draft.body, None)
                .map_err(|e| ServiceError::Message(e.to_string()))?;
            Ok(WriteOutcome::NativeId(SourceId::new(ServiceId::new(self.id), post.id)))
        })
    }
}

acp_rs_plugin::impl_default_caps!(OrgService, except Reader, Post);

/// Text of the manifest a dry-run would write. Nothing is written here.
pub fn preview(c: &rusqlite::Connection, slug: &str) -> Result<String> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let posts = list_posts(c, org.id)?;
    let assets = access::list_assets(c, org.id)?;
    let mut lines = vec![
        format!("id = \"org-{slug}\""),
        format!("name = {:?}", org.name),
        format!("endpoint = \"http://127.0.0.1:0\""),
        "reader = true".into(),
        format!("post = {}", org.allow_remote_post),
    ];
    if org.visibility != Visibility::Public {
        lines.push("a private organization exposes nothing".into());
    } else {
        for post in posts.iter().filter(|p| public_row(&org, p, false)) {
            lines.push(format!("post {}", post.title));
        }
        for asset in assets.iter().filter(|a| access::asset_public(&org, a, false)) {
            lines.push(format!("asset {}", asset.name));
        }
        if org.gated {
            lines.push("gated: an anonymous caller gets an empty page".into());
        }
    }
    Ok(lines.join("\n"))
}

/// Mark the organization published and listen until `shutdown`. Loopback only.
pub fn serve_published(db: &Path, services: &Path, slug: &str, addr: SocketAddr, ready: &std::sync::mpsc::Sender<SocketAddr>, shutdown: &AtomicBool) -> Result<()> {
    if !addr.ip().is_loopback() {
        bail!("an organization service binds 127.0.0.1 only");
    }
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if org.visibility != Visibility::Public {
        bail!("a private organization is not published");
    }
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let bound = listener.local_addr()?;
    write_manifest(services, &org, &bound)?;
    set_flag(&c, slug, "published", true)?;
    let _ = ready.send(bound);
    println!("published org-{} at http://{bound}/", org.slug);
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => { let _ = answer(stream, db, slug); }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn write_manifest(dir: &Path, org: &Org, bound: &SocketAddr) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("org-{}.toml", org.slug));
    let body = format!(
        "id = \"org-{}\"\nname = {:?}\nslug = {:?}\nendpoint = \"http://{bound}\"\nreader = true\npost = {}\n",
        org.slug, org.name, org.slug, org.allow_remote_post
    );
    std::fs::write(&path, body)?;
    Ok(path)
}

fn set_flag(c: &rusqlite::Connection, slug: &str, column: &str, on: bool) -> Result<()> {
    if column != "published" && column != "allow_remote_post" {
        bail!("unknown service flag");
    }
    c.execute(&format!("UPDATE organizations SET {column} = ?1, updated_at = ?2 WHERE slug = ?3"), params![i64::from(on), super::now(), slug])?;
    Ok(())
}

/// Turn remote posts on or off. The manifest is rewritten when it exists.
pub fn set_remote_posts(c: &rusqlite::Connection, services: &Path, slug: &str, on: bool) -> Result<()> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if !org.published {
        bail!("not published");
    }
    set_flag(c, slug, "allow_remote_post", on)?;
    let path = services.join(format!("org-{slug}.toml"));
    if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        let next = if on {
            text.replace("post = false", "post = true")
        } else {
            text.replace("post = true", "post = false")
        };
        std::fs::write(path, next)?;
    }
    Ok(())
}

/// Delete the manifest and clear the published flag. Posts stay.
pub fn unpublish(c: &rusqlite::Connection, services: &Path, slug: &str) -> Result<()> {
    set_flag(c, slug, "published", false)?;
    let path = services.join(format!("org-{slug}.toml"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn answer(mut stream: TcpStream, db: &Path, slug: &str) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).unwrap_or(0);
    buf.truncate(n);
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.split("\r\n");
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let mut identity = String::new();
    let mut nonce = String::new();
    let mut signature = String::new();
    let mut length = 0usize;
    for line in text.split("\r\n").skip(1) {
        if line.is_empty() { break; }
        let Some((name, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "x-acp-identity" => identity = value.to_string(),
            "x-acp-nonce" => nonce = value.to_string(),
            "x-acp-signature" => signature = value.to_string(),
            "content-length" => length = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").as_bytes();
    let body = if length > 0 && body.len() >= length { &body[..length] } else { body };
    let (status, payload) = dispatch(db, slug, method, path, Caller { identity: &identity, nonce: &nonce, signature: &signature }, body);
    let head = format!("HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
    stream.write_all(head.as_bytes())?;
    stream.write_all(payload.as_bytes())?;
    Ok(())
}

struct Caller<'a> {
    identity: &'a str,
    nonce: &'a str,
    signature: &'a str,
}

fn dispatch(db: &Path, slug: &str, method: &str, path: &str, caller: Caller<'_>, body: &[u8]) -> (&'static str, String) {
    let c = match org_conn(db) {
        Ok(c) => c,
        Err(e) => return ("500 Internal Server Error", json!({"error": e.to_string()}).to_string()),
    };
    let _ = c.busy_timeout(Duration::from_secs(2));
    let result = if method == "GET" && path == "/v1/manifest" {
        manifest_json(&c, slug)
    } else if method == "GET" && path == "/v1/page" {
        page_json(&c, slug, caller.identity, caller.nonce, caller.signature)
    } else if method == "POST" && path == "/v1/post" {
        write_json(&c, slug, body, WriteKind::Post)
    } else if method == "POST" && path == "/v1/reply" {
        write_json(&c, slug, body, WriteKind::Reply)
    } else if method == "POST" && path == "/v1/attach" {
        write_json(&c, slug, body, WriteKind::Attach)
    } else {
        Err(anyhow!("not found"))
    };
    match result {
        Ok(value) => ("200 OK", value.to_string()),
        Err(e) => ("403 Forbidden", json!({"error": e.to_string()}).to_string()),
    }
}

fn manifest_json(c: &rusqlite::Connection, slug: &str) -> Result<Value> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if !org.published || org.visibility != Visibility::Public {
        bail!("not published");
    }
    Ok(json!({
        "id": format!("org-{slug}"),
        "name": org.name,
        "slug": slug,
        "reader": true,
        "post": org.allow_remote_post,
    }))
}

fn page_json(c: &rusqlite::Connection, slug: &str, identity: &str, nonce: &str, signature: &str) -> Result<Value> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if org.visibility != Visibility::Public {
        return Ok(json!({"availability": "off", "items": []}));
    }
    let member = if identity.is_empty() {
        false
    } else {
        member_signature(c, &org, identity, nonce, signature)?
    };
    if org.gated && !member {
        return Ok(json!({"availability": "gated", "items": []}));
    }
    let posts = list_posts(c, org.id)?;
    let assets = access::list_assets(c, org.id)?;
    let mut items = Vec::new();
    for post in posts.iter().filter(|p| public_row(&org, p, member)) {
        items.push(json!({"id": post.id, "title": post.title, "body": post.body, "kind": "post"}));
    }
    for asset in assets.iter().filter(|a| access::asset_public(&org, a, member)) {
        items.push(json!({"id": asset.id, "title": asset.name, "body": "", "kind": "asset"}));
    }
    Ok(json!({"availability": "ready", "items": items}))
}

fn member_signature(c: &rusqlite::Connection, org: &Org, identity: &str, nonce: &str, signature: &str) -> Result<bool> {
    access::check_nonce(nonce)?;
    let sig = data_encoding::HEXLOWER.decode(signature.trim().as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    access::verify(identity, &access::read_message(nonce, &org.slug), &sig)?;
    let members = access::members(c, org.id)?;
    let Some(member) = members.into_iter().find(|m| m.identity_pub == identity) else {
        return Ok(false);
    };
    if member.status != access::MemberStatus::Active {
        return Ok(false);
    }
    if member.role == "Owner" {
        return Ok(true);
    }
    let roles = access::roles(c, org.id)?;
    let Some(role) = roles.iter().find(|r| r.name == member.role) else { return Ok(false) };
    Ok(role.permissions.split_whitespace().any(|p| p == "view"))
}

#[derive(Clone, Copy)]
enum WriteKind { Post, Reply, Attach }

fn write_json(c: &rusqlite::Connection, slug: &str, body: &[u8], kind: WriteKind) -> Result<Value> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if !org.allow_remote_post {
        bail!("remote posts are off");
    }
    let v: Value = serde_json::from_slice(body).map_err(|_| anyhow!("bad write"))?;
    let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("");
    let text = v.get("body").and_then(|t| t.as_str()).unwrap_or("");
    let source = v.get("source").and_then(|t| t.as_str()).unwrap_or("service");
    let parent = v.get("parent").and_then(|t| t.as_str()).unwrap_or("");
    let name = v.get("name").and_then(|t| t.as_str()).unwrap_or("");
    let byte_len = v.get("byte_len").and_then(|t| t.as_u64()).unwrap_or(0);
    let identity = v.get("identity").and_then(|t| t.as_str()).unwrap_or("");
    if identity.is_empty() {
        let row = match kind {
            WriteKind::Post => access::submit_guest_post(c, slug, source, title, text, None)?.id,
            WriteKind::Reply => access::submit_guest_reply(c, slug, source, parent, title, text, None)?.id,
            WriteKind::Attach => access::submit_guest_attachment(c, slug, source, parent, name, byte_len, None)?.id,
        };
        return Ok(json!({"id": row, "author": ANONYMOUS_AUTHOR}));
    }
    let nonce = v.get("nonce").and_then(|t| t.as_str()).unwrap_or("");
    let signature = data_encoding::HEXLOWER.decode(v.get("signature").and_then(|t| t.as_str()).unwrap_or("").as_bytes()).map_err(|_| anyhow!("signature rejected"))?;
    let row = match kind {
        WriteKind::Post => access::submit_signed_post(c, slug, identity, nonce, title, text, &signature, None, source)?.id,
        WriteKind::Reply => access::submit_signed_reply(c, slug, identity, nonce, parent, title, text, &signature, None, source)?.id,
        WriteKind::Attach => access::submit_signed_attachment(c, slug, identity, nonce, parent, name, &signature, None, source, byte_len)?.id,
    };
    Ok(json!({"id": row, "author": identity}))
}

fn visible_items(db: &Path, slug: &str, service: &'static str, member: bool) -> Result<Vec<ItemRec>> {
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if org.visibility != Visibility::Public || (org.gated && !member) {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for post in list_posts(&c, org.id)?.into_iter().filter(|p| public_row(&org, p, member)) {
        let mut item = ItemRec::new(SourceId::new(ServiceId::new(service), post.id), post.body);
        item.title = post.title;
        item.kind = "post".into();
        item.nick = post.author_identity;
        items.push(item);
    }
    Ok(items)
}

/// Fetch a published manifest and remember it under `services`.
pub fn add_remote(services: &Path, endpoint: &str) -> Result<String> {
    let url = loopback_http(endpoint)?;
    let body = http(&url, "GET", "/v1/manifest", &[], &[])?;
    if !body.contains("200") {
        bail!("the service did not answer");
    }
    let json = body.split("\r\n\r\n").nth(1).unwrap_or("");
    let v: Value = serde_json::from_str(json)?;
    let slug = v.get("slug").and_then(|s| s.as_str()).unwrap_or("").to_string();
    if slug.is_empty() {
        bail!("the manifest has no slug");
    }
    std::fs::create_dir_all(services)?;
    let path = services.join(format!("added-{slug}.toml"));
    std::fs::write(&path, format!("slug = {slug:?}\nendpoint = {endpoint:?}\n"))?;
    Ok(slug)
}

/// Read the public page of an added service. Returns the response body.
pub fn browse(services: &Path, slug: &str, headers: &[(&str, &str)]) -> Result<String> {
    let path = services.join(format!("added-{slug}.toml"));
    let text = std::fs::read_to_string(&path).with_context(|| format!("no added service {slug}"))?;
    let endpoint = text.lines().find_map(|l| l.strip_prefix("endpoint = ").map(|v| v.trim().trim_matches('"').to_string())).ok_or_else(|| anyhow!("no endpoint"))?;
    let url = loopback_http(&endpoint)?;
    let raw = http(&url, "GET", "/v1/page", headers, &[])?;
    if !raw.starts_with("HTTP/1.0 200") {
        let err = raw.split("\r\n\r\n").nth(1).unwrap_or(raw.trim());
        bail!("{err}");
    }
    Ok(raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

fn loopback_http(raw: &str) -> Result<url::Url> {
    let url = url::Url::parse(raw).with_context(|| format!("bad address {raw}"))?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
        bail!("an organization service binds 127.0.0.1 only");
    }
    Ok(url)
}

fn http(url: &url::Url, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<String> {
    let addr = format!("{}:{}", url.host_str().unwrap_or("127.0.0.1"), url.port().unwrap_or(80));
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (name, value) in headers {
        req.push_str(name);
        req.push_str(": ");
        req.push_str(value);
        req.push_str("\r\n");
    }
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// POST a JSON write and return the response body.
pub fn post_json(endpoint: &str, path: &str, body: &Value) -> Result<String> {
    let url = loopback_http(endpoint)?;
    let bytes = body.to_string();
    let raw = http(&url, "POST", path, &[], bytes.as_bytes())?;
    Ok(raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::Service;
    use std::sync::Arc;
    use std::task::{Context, Poll};

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "acp-org-svc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn person() -> (acp_red_star_node_core::node::NodeSecret, String) {
        let key = acp_red_star_node_core::node::NodeSecret::generate();
        let hex = data_encoding::HEXLOWER.encode(&key.sign_pub);
        (key, hex)
    }

    fn drive<T>(fut: impl std::future::Future<Output = T>) -> T {
        let mut fut = Box::pin(fut);
        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("the adapter future waited"),
        }
    }

    #[test]
    fn a_second_home_browses_only_public_rows_and_posts_stay_off() {
        assert!(crate::service::Registry::builtins().services().is_empty());
        let home = dir();
        let db = home.join("db.sqlite");
        let services = home.join("services");
        let c = org_conn(&db).unwrap();
        let (owner, owner_hex) = person();
        crate::orgs::create_org(&c, "hold", "Rebel Hold", "secret-description", Visibility::Private, Visibility::Private, &owner_hex).unwrap();
        crate::orgs::add_post(&c, "hold", "Secret", "hidden-body", None).unwrap();
        let preview = preview(&c, "hold").unwrap();
        assert!(preview.contains("org-hold"));
        assert!(preview.contains("exposes nothing"));
        assert!(!preview.contains("hidden-body"));
        let err = serve_published(&db, &services, "hold", "127.0.0.1:0".parse().unwrap(), &std::sync::mpsc::channel().0, &AtomicBool::new(true)).unwrap_err();
        assert!(err.to_string().contains("private"));
        assert!(!services.join("org-hold.toml").exists());

        crate::orgs::update_org(&c, "hold", &crate::orgs::OrgEdit {
            name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: None,
        }).unwrap();
        let (_org, marked) = crate::orgs::add_post(&c, "hold", "Marked", "marked-body", Some(Visibility::Public)).unwrap();
        let anon = RuntimeCtx { enabled: true, logged_in: false, limited: None };
        let svc = OrgService::load(&db, "hold").unwrap();
        assert_eq!(svc.availability(CapabilityId::Reader, &anon), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Post, &anon), Availability::Off);
        assert!(!svc.implemented().contains(&CapabilityId::Post));
        let resolved = Resolved { target: acp_rs_plugin::Target::Site, source_id: SourceId::new(svc.service_id(), "hold") };
        let mut budget = Budget::unlimited();
        let page = drive(acp_rs_plugin::Reader::page(&svc, &resolved, &Cursor::default(), None, &mut budget)).unwrap();
        assert!(page.items.is_empty());
        let refused = drive(acp_rs_plugin::Post::publish(&svc, &Draft::text("nope"))).unwrap();
        assert_eq!(refused, WriteOutcome::NotReady(Availability::Off));

        assert!(serve_published(&db, &services, "hold", "0.0.0.0:9".parse().unwrap(), &std::sync::mpsc::channel().0, &AtomicBool::new(true)).is_err());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let db2 = db.clone();
        let svc_dir = services.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || serve_published(&db2, &svc_dir, "hold", "127.0.0.1:0".parse().unwrap(), &tx, &flag).unwrap());
        let bound = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let endpoint = format!("http://{bound}");
        let other = dir().join("services");
        let slug = add_remote(&other, &endpoint).unwrap();
        assert_eq!(slug, "hold");
        let gated = browse(&other, "hold", &[]).unwrap();
        assert!(gated.contains("\"availability\":\"gated\""));
        assert!(!gated.contains("Marked"));
        assert!(!gated.contains("hidden-body"));
        assert!(!gated.contains("Secret"));
        assert!(!gated.contains("secret-description"));

        access::set_gated(&c, get_org(&c, "hold").unwrap().unwrap().id, false).unwrap();
        let open = browse(&other, "hold", &[]).unwrap();
        assert!(open.contains("marked-body"));
        assert!(!open.contains("hidden-body"));
        let off = post_json(&endpoint, "/v1/post", &json!({"title": "Guest", "body": "guest-body", "source": "203.0.113.9"})).unwrap();
        assert!(off.contains("remote posts are off"));
        set_remote_posts(&c, &services, "hold", true).unwrap();
        let wrote = post_json(&endpoint, "/v1/post", &json!({"title": "Guest", "body": "guest-body", "source": "203.0.113.9"})).unwrap();
        assert!(wrote.contains(ANONYMOUS_AUTHOR));
        assert!(list_posts(&c, get_org(&c, "hold").unwrap().unwrap().id).unwrap().iter().any(|p| p.body == "guest-body" && p.author_identity == ANONYMOUS_AUTHOR));

        let (viewer, viewer_hex) = person();
        access::invite_member(&c, get_org(&c, "hold").unwrap().unwrap().id, &owner_hex, &viewer_hex).unwrap();
        let nonce = "cc33cc33cc33cc33";
        let msg = access::post_message(nonce, "hold", "Nope", "viewer-body");
        let sig = data_encoding::HEXLOWER.encode(&viewer.sign(&msg));
        let denied = post_json(&endpoint, "/v1/post", &json!({"title": "Nope", "body": "viewer-body", "source": "203.0.113.10", "identity": viewer_hex, "nonce": nonce, "signature": sig})).unwrap();
        assert!(denied.contains("missing permission"));
        assert!(!denied.contains("hidden-body"));

        access::set_gated(&c, get_org(&c, "hold").unwrap().unwrap().id, true).unwrap();
        let hidden = browse(&other, "hold", &[]).unwrap();
        assert!(hidden.contains("gated"));
        assert!(!hidden.contains("marked-body"));
        let read_nonce = "dd44dd44dd44dd44";
        let read = access::read_message(read_nonce, "hold");
        let read_sig = data_encoding::HEXLOWER.encode(&owner.sign(&read));
        let seen = browse(&other, "hold", &[("X-Acp-Identity", &owner_hex), ("X-Acp-Nonce", read_nonce), ("X-Acp-Signature", &read_sig)]).unwrap();
        assert!(seen.contains("marked-body"));
        assert!(seen.contains(&marked.id));
        assert!(!seen.contains("hidden-body"));

        let bad = browse(&other, "hold", &[("X-Acp-Identity", &owner_hex), ("X-Acp-Nonce", read_nonce), ("X-Acp-Signature", "00")]).unwrap_err();
        assert!(bad.to_string().contains("signature rejected") || format!("{bad:#}").contains("signature rejected"));
        unpublish(&c, &services, "hold").unwrap();
        assert!(!services.join("org-hold.toml").exists());
        stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}
