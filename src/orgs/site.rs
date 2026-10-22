//! Public website for an organization: a folder of HTML, Markdown, and a feed,
//! and the same pages on `127.0.0.1`.
//!
//! Replication of this folder onto another node is 3.49.0. An Owner archive
//! replicates as an update, not a delete.

use super::assets::absolute_path;
use super::template::{self, ThemePaint};
use super::{Audience, Visibility, get_org, list_posts, public_row};
use crate::prelude::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Replication between nodes is a later minor. This site does not send it.
pub const REPLICATION_NOTE: &str = "Replication of this site between nodes is 3.49.0. An Owner archive replicates as an update, not a delete.";

/// One exported site.
#[derive(Clone, Debug)]
pub struct Site {
    /// Organization slug.
    pub slug: String,
    /// Front page.
    pub index: String,
    /// Public posts.
    pub posts: Vec<SitePost>,
    /// Public files that have bytes.
    pub files: Vec<SiteFile>,
    /// `feed.json`.
    pub feed: String,
    /// Rows left out of the public site.
    pub omitted: usize,
    /// Why the site is empty or partial.
    pub note: String,
}

/// A public post page and its Markdown twin.
#[derive(Clone, Debug)]
pub struct SitePost {
    /// Post id.
    pub id: String,
    /// HTML page.
    pub html: String,
    /// Markdown file.
    pub markdown: String,
}

/// A public download.
#[derive(Clone, Debug)]
pub struct SiteFile {
    /// Asset id.
    pub id: String,
    /// Download page.
    pub html: String,
    /// File bytes. Empty when the asset recorded no file.
    pub bytes: Vec<u8>,
}

/// Build the public site. A private organization contributes no names and no posts.
pub fn build(c: &Connection, db: &Path, slug: &str) -> Result<Site> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let posts = list_posts(c, org.id)?;
    let assets = super::access::list_assets(c, org.id)?;
    let omitted = posts.iter().filter(|p| !public_row(&org, p, false)).count()
        + assets.iter().filter(|a| !super::access::asset_public(&org, a, false)).count();
    if org.visibility != Visibility::Public {
        return Ok(empty(slug, omitted, "this organization has no public pages"));
    }
    if org.gated {
        let index = format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body><h1>{}</h1><p>Register and login in the Red Star app.</p></body></html>\n",
            escape(&org.name),
            escape(&org.name)
        );
        return Ok(Site {
            slug: slug.to_string(),
            index,
            posts: Vec::new(),
            files: Vec::new(),
            feed: feed_json(slug, &[]),
            omitted,
            note: "gated: landing page only".into(),
        });
    }
    let css = ThemePaint::neutral().css();
    let index = template::render_org(c, slug, &org.name, "", &css, Audience::Public)?;
    let mut site_posts = Vec::new();
    let mut feed_posts = Vec::new();
    for post in &posts {
        if !public_row(&org, post, false) {
            continue;
        }
        site_posts.push(SitePost {
            id: post.id.clone(),
            html: format!(
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>\n",
                escape(&post.title),
                escape(&post.title),
                escape(&post.body)
            ),
            markdown: format!("# {}\n\n{}\n", post.title, post.body),
        });
        feed_posts.push((post.id.clone(), post.title.clone(), post.body.clone()));
    }
    let mut files = Vec::new();
    for asset in &assets {
        if !super::access::asset_public(&org, asset, false) {
            continue;
        }
        let bytes = absolute_path(db, asset).and_then(|p| std::fs::read(p).ok()).unwrap_or_default();
        files.push(SiteFile {
            id: asset.id.clone(),
            html: format!(
                "<!DOCTYPE html><html><body><p><a href=\"../downloads/{}\">{}</a></p></body></html>\n",
                escape(&asset.id),
                escape(&asset.name)
            ),
            bytes,
        });
    }
    Ok(Site {
        slug: slug.to_string(),
        index,
        posts: site_posts,
        files,
        feed: feed_json(slug, &feed_posts),
        omitted,
        note: "public".into(),
    })
}

fn empty(slug: &str, omitted: usize, note: &str) -> Site {
    Site {
        slug: slug.into(),
        index: "<!DOCTYPE html><html><body><p>This organization has no public pages.</p></body></html>\n".into(),
        posts: Vec::new(),
        files: Vec::new(),
        feed: feed_json(slug, &[]),
        omitted,
        note: note.into(),
    }
}

fn feed_json(slug: &str, posts: &[(String, String, String)]) -> String {
    let items: Vec<Value> = posts.iter().map(|(id, title, body)| json!({"id": id, "title": title, "body": body})).collect();
    serde_json::to_string_pretty(&json!({"slug": slug, "posts": items})).unwrap_or_else(|_| "{}".into())
}

/// Write the site. Does not create the directory's parents beyond `dir` itself.
pub fn write(site: &Site, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join("posts"))?;
    std::fs::create_dir_all(dir.join("downloads"))?;
    std::fs::write(dir.join("index.html"), &site.index)?;
    std::fs::write(dir.join("feed.json"), &site.feed)?;
    for post in &site.posts {
        std::fs::write(dir.join("posts").join(format!("{}.html", post.id)), &post.html)?;
        std::fs::write(dir.join("posts").join(format!("{}.md", post.id)), &post.markdown)?;
    }
    for file in &site.files {
        std::fs::write(dir.join("downloads").join(format!("{}.html", file.id)), &file.html)?;
        if !file.bytes.is_empty() {
            std::fs::write(dir.join("downloads").join(&file.id), &file.bytes)?;
        }
    }
    Ok(())
}

/// Serve `site` on a loopback address until `shutdown` is set. Sends the bound address first.
pub fn serve_until(site: &Site, addr: SocketAddr, ready: &std::sync::mpsc::Sender<SocketAddr>, shutdown: &AtomicBool) -> Result<()> {
    if !addr.ip().is_loopback() {
        bail!("an organization site binds 127.0.0.1 only");
    }
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let bound = listener.local_addr()?;
    let _ = ready.send(bound);
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => { let _ = answer(stream, site); }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Serve until the process is stopped. Loopback only.
pub fn serve(site: &Site, addr: SocketAddr) -> Result<SocketAddr> {
    if !addr.ip().is_loopback() {
        bail!("an organization site binds 127.0.0.1 only");
    }
    let listener = TcpListener::bind(addr)?;
    let bound = listener.local_addr()?;
    println!("serving {} at http://{bound}/", site.slug);
    println!("{REPLICATION_NOTE}");
    for stream in listener.incoming() {
        let _ = answer(stream?, site);
    }
    Ok(bound)
}

fn answer(mut stream: TcpStream, site: &Site) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (status, kind, body) = route(site, path);
    let head = format!("HTTP/1.0 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)?;
    Ok(())
}

fn route(site: &Site, path: &str) -> (&'static str, &'static str, Vec<u8>) {
    if path == "/" || path == "/index.html" {
        return ("200 OK", "text/html; charset=utf-8", site.index.as_bytes().to_vec());
    }
    if path == "/feed.json" {
        return ("200 OK", "application/json", site.feed.as_bytes().to_vec());
    }
    if let Some(id) = path.strip_prefix("/posts/").and_then(|p| p.strip_suffix(".html")) {
        if let Some(post) = site.posts.iter().find(|p| p.id == id) {
            return ("200 OK", "text/html; charset=utf-8", post.html.as_bytes().to_vec());
        }
    }
    if let Some(id) = path.strip_prefix("/posts/").and_then(|p| p.strip_suffix(".md")) {
        if let Some(post) = site.posts.iter().find(|p| p.id == id) {
            return ("200 OK", "text/markdown; charset=utf-8", post.markdown.as_bytes().to_vec());
        }
    }
    if let Some(rest) = path.strip_prefix("/downloads/") {
        if let Some(id) = rest.strip_suffix(".html") {
            if let Some(file) = site.files.iter().find(|f| f.id == id) {
                return ("200 OK", "text/html; charset=utf-8", file.html.as_bytes().to_vec());
            }
        } else if let Some(file) = site.files.iter().find(|f| f.id == rest) {
            return ("200 OK", "application/octet-stream", file.bytes.clone());
        }
    }
    ("404 Not Found", "text/plain; charset=utf-8", b"not found\n".to_vec())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orgs::Visibility;
    use std::io::{Read, Write};
    use std::sync::Arc;

    fn conn() -> (std::path::PathBuf, Connection) {
        let d = std::env::temp_dir().join(format!(
            "acp-site-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        let db = d.join("db.sqlite");
        (db.clone(), crate::orgs::org_conn(&db).unwrap())
    }

    fn person() -> String {
        data_encoding::HEXLOWER.encode(&acp_red_star_node_core::node::NodeSecret::generate().sign_pub)
    }

    #[test]
    fn a_private_org_exports_nothing_and_a_public_one_serves_only_public_rows() {
        assert!(REPLICATION_NOTE.contains("3.49.0"));
        assert!(REPLICATION_NOTE.contains("update"));
        assert!(!REPLICATION_NOTE.contains("prepared, gated:"));
        let (db, c) = conn();
        let owner = person();
        crate::orgs::create_org(&c, "hold", "Rebel Hold", "secret-description", Visibility::Private, Visibility::Private, &owner).unwrap();
        crate::orgs::add_post(&c, "hold", "Secret", "hidden-body", None).unwrap();
        let site = build(&c, &db, "hold").unwrap();
        assert!(!site.index.contains("hidden-body"));
        assert!(!site.index.contains("Secret"));
        assert!(!site.index.contains("secret-description"));
        assert!(site.posts.is_empty());
        assert!(site.feed.contains("\"posts\": []") || site.feed.contains("\"posts\":[]"));
        let dir = db.parent().unwrap().join("out");
        write(&site, &dir).unwrap();
        assert!(!std::fs::read_to_string(dir.join("index.html")).unwrap().contains("hidden-body"));

        crate::orgs::update_org(&c, "hold", &crate::orgs::OrgEdit {
            name: None, description: None, visibility: Some(Visibility::Public), default_post_visibility: None,
        }).unwrap();
        let gated = build(&c, &db, "hold").unwrap();
        assert!(gated.index.contains("Rebel Hold"));
        assert!(gated.index.contains("Register"));
        assert!(!gated.index.contains("Secret"));
        assert!(gated.posts.is_empty());
        assert!(!gated.feed.contains("hidden-body"));

        let org = get_org(&c, "hold").unwrap().unwrap();
        crate::orgs::access::set_gated(&c, org.id, false).unwrap();
        let (_org, marked) = crate::orgs::add_post(&c, "hold", "Marked", "marked-body", Some(Visibility::Public)).unwrap();
        let src = db.parent().unwrap().join("banner.txt");
        std::fs::write(&src, b"file-bytes").unwrap();
        let asset = crate::orgs::assets::add_file(&c, &db, "hold", &owner, &src).unwrap();
        crate::orgs::assets::set_visibility(&c, &owner, &asset.id, Visibility::Public).unwrap();
        let open = build(&c, &db, "hold").unwrap();
        assert!(open.index.contains("Marked"));
        assert!(!open.index.contains("hidden-body"));
        assert!(open.posts.iter().any(|p| p.markdown.contains("marked-body")));
        assert!(!open.posts.iter().any(|p| p.markdown.contains("hidden-body")));
        assert!(open.files.iter().any(|f| f.bytes == b"file-bytes"));
        let out = db.parent().unwrap().join("pub");
        write(&open, &out).unwrap();
        assert!(std::fs::read(out.join("downloads").join(&asset.id)).unwrap() == b"file-bytes");
        assert!(std::fs::read_to_string(out.join("posts").join(format!("{}.md", marked.id))).unwrap().contains("marked-body"));

        assert!(serve_until(&open, "0.0.0.0:9".parse().unwrap(), &std::sync::mpsc::channel().0, &AtomicBool::new(true)).is_err());
        let stop = Arc::new(AtomicBool::new(false));
        let site = open.clone();
        let flag = stop.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || serve_until(&site, "127.0.0.1:0".parse().unwrap(), &tx, &flag).unwrap());
        let bound = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut sock = TcpStream::connect(bound).unwrap();
        sock.write_all(b"GET /feed.json HTTP/1.0\r\n\r\n").unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("200"));
        assert!(text.contains("marked-body"));
        assert!(!text.contains("hidden-body"));
        let mut sock = TcpStream::connect(bound).unwrap();
        sock.write_all(b"GET /posts/missing.html HTTP/1.0\r\n\r\n").unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("404"));
        stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}
