//! An in-memory [`crate::Host`] for a service crate's tests.
//!
//! Enable the `test-host` feature. This module does not open a database and it
//! does not talk to the network. HTTP returns the scripted response from
//! [`MemHost::script_http`].

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::host::*;
use crate::records::{BodyState, ItemRec, NoticeRec, Page, SourceId};

struct Mark {
    remote: bool,
    deleted: bool,
}

struct DraftRow {
    title: String,
    body: String,
    thread: String,
    keep: bool,
    status: String,
}

struct HttpScript {
    status: u16,
    body: Vec<u8>,
    content_type: String,
    /// Empty means the request URL. A test sets this to the URL after a redirect.
    url: String,
    headers: Vec<HttpHeader>,
    last: String,
}

struct Memory {
    home: String,
    base_url: String,
    items: BTreeMap<String, ItemRec>,
    secrets: BTreeMap<String, Vec<u8>>,
    bookmarks: BTreeMap<String, Mark>,
    /// `(service, post_id)` → site bookmark id.
    sites: BTreeMap<(String, String), String>,
    outbox: BTreeMap<String, DraftRow>,
    prefs: BTreeMap<String, String>,
    notices: Vec<NoticeRec>,
    cookies: BTreeMap<String, Vec<CookieRec>>,
    http: HttpScript,
    research: Vec<String>,
}

/// In-memory host. Clone shares the same store.
#[derive(Clone)]
pub struct MemHost {
    inner: std::sync::Arc<Mutex<Memory>>,
}

impl MemHost {
    /// An empty store. `home` is the directory [`Host::home`] reports.
    pub fn new(home: impl Into<String>) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(Memory {
                home: home.into(),
                base_url: "http://127.0.0.1".into(),
                items: BTreeMap::new(),
                secrets: BTreeMap::new(),
                bookmarks: BTreeMap::new(),
                sites: BTreeMap::new(),
                outbox: BTreeMap::new(),
                prefs: BTreeMap::new(),
                notices: Vec::new(),
                cookies: BTreeMap::new(),
                http: HttpScript {
                    status: 200,
                    body: Vec::new(),
                    content_type: "text/plain".into(),
                    url: String::new(),
                    headers: Vec::new(),
                    last: String::new(),
                },
                research: Vec::new(),
            })),
        }
    }

    /// The next HTTP call returns this response and does not open a socket.
    ///
    /// This clears a scripted final URL and scripted headers from an earlier call.
    pub fn script_http(&self, status: u16, body: impl Into<Vec<u8>>, content_type: &str) {
        let mut mem = self.lock();
        mem.http.status = status;
        mem.http.body = body.into();
        mem.http.content_type = content_type.to_string();
        mem.http.url.clear();
        mem.http.headers.clear();
    }

    /// Headers for the next scripted response.
    ///
    /// A `Set-Cookie` is merged into that service's jar when the call runs.
    /// The response lists the cookie name and keeps the header. [`Debug`] on
    /// [`HttpResponse`] redacts the value.
    pub fn script_headers(&self, headers: impl IntoIterator<Item = HttpHeader>) {
        self.lock().http.headers = headers.into_iter().collect();
    }

    /// Final URL the next scripted response reports. Empty keeps the request URL.
    pub fn script_final_url(&self, url: &str) {
        self.lock().http.url = url.to_string();
    }

    /// The last HTTP request line, including headers the caller passed.
    pub fn last_http(&self) -> String {
        self.lock().http.last.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl Host for MemHost {
    fn home(&self) -> Result<String, HostError> {
        Ok(self.lock().home.clone())
    }

    fn database_path(&self) -> Result<String, HostError> {
        Ok(format!("{}/.acp/ai/data/rs.database", self.lock().home))
    }

    fn base_url(&self) -> Result<String, HostError> {
        Ok(self.lock().base_url.clone())
    }

    fn email(&self) -> Result<String, HostError> {
        Ok(self.lock().prefs.get("email").cloned().unwrap_or_default())
    }

    fn save_page(&self, page: &Page) -> Result<usize, HostError> {
        let mut mem = self.lock();
        let mut n = 0;
        for item in &page.items {
            if item.source_id.native.is_empty() {
                return Err(HostError::new("an item needs a native id"));
            }
            mem.items.insert(item.source_id.native.clone(), item.clone());
            n += 1;
        }
        for segment in &page.segments {
            let item = segment_item(segment);
            if item.source_id.native.is_empty() {
                return Err(HostError::new("an item needs a native id"));
            }
            mem.items.insert(item.source_id.native.clone(), item);
            n += 1;
        }
        Ok(n)
    }

    fn load_item(&self, id: &SourceId) -> Result<Option<ItemRec>, HostError> {
        Ok(self.lock().items.get(&id.native).cloned())
    }

    fn known_ids(&self) -> Result<KnownIds, HostError> {
        let mem = self.lock();
        let mut ids: Vec<String> = mem
            .items
            .values()
            .filter(|item| item.body_state != BodyState::Truncated && !item.body.is_empty())
            .map(|item| item.source_id.native.clone())
            .collect();
        ids.sort();
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for item in mem.items.values() {
            if let Some(parent) = &item.parent {
                *counts.entry(parent.native.clone()).or_default() += 1;
            }
        }
        let reply_counts = counts
            .into_iter()
            .map(|(thread_id, replies)| ThreadReplies { thread_id, replies })
            .collect();
        Ok(KnownIds { ids, reply_counts })
    }

    fn store_secret(&self, field: &str, material: &[u8]) -> Result<(), HostError> {
        if field.is_empty() {
            return Err(HostError::new("a secret needs a field name"));
        }
        self.lock().secrets.insert(field.to_string(), material.to_vec());
        Ok(())
    }

    fn load_secret(&self, field: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.lock().secrets.get(field).cloned())
    }

    fn list_bookmarks(&self) -> Result<Vec<String>, HostError> {
        Ok(self
            .lock()
            .bookmarks
            .iter()
            .filter(|(_, mark)| !mark.deleted)
            .map(|(id, _)| id.clone())
            .collect())
    }

    fn add_bookmark(&self, id: &str) -> Result<(), HostError> {
        self.lock().bookmarks.insert(
            id.to_string(),
            Mark {
                remote: false,
                deleted: false,
            },
        );
        Ok(())
    }

    fn remove_bookmark(&self, id: &str) -> Result<(), HostError> {
        let mut mem = self.lock();
        if let Some(mark) = mem.bookmarks.get_mut(id) {
            if mark.remote {
                mark.deleted = true;
            } else {
                mem.bookmarks.remove(id);
            }
        }
        Ok(())
    }

    fn bookmark_post(&self, service: &str, post_id: &str, on: bool) -> Result<(), HostError> {
        if service.is_empty() || post_id.is_empty() {
            return Err(HostError::new("a bookmark needs a service and a post id"));
        }
        if on {
            self.add_bookmark(post_id)?;
            self.lock().sites.insert((service.to_string(), post_id.to_string()), post_id.to_string());
        } else {
            self.lock().sites.remove(&(service.to_string(), post_id.to_string()));
            self.remove_bookmark(post_id)?;
        }
        Ok(())
    }

    fn list_outbox(&self) -> Result<Vec<String>, HostError> {
        let mem = self.lock();
        let mut lines: Vec<String> = mem
            .outbox
            .iter()
            .map(|(id, row)| {
                let action = if row.thread.is_empty() { "new_post" } else { "reply" };
                let state = if row.keep { "local" } else { row.status.as_str() };
                format!(
                    "outbox Q-{id} status={state} action={action} title={} bytes={}",
                    row.title,
                    row.body.len()
                )
            })
            .collect();
        if lines.is_empty() {
            lines.push("outbox none".into());
        }
        Ok(lines)
    }

    fn save_outbox(
        &self,
        id: &str,
        title: &str,
        body: &str,
        thread: &str,
        keep: bool,
    ) -> Result<String, HostError> {
        if id.trim().is_empty() || body.trim().is_empty() {
            return Err(HostError::new("a draft needs an id and a body"));
        }
        let state = if keep { "local" } else { "draft" };
        self.lock().outbox.insert(
            id.trim().to_string(),
            DraftRow {
                title: title.to_string(),
                body: body.trim().to_string(),
                thread: thread.trim().to_string(),
                keep,
                status: state.to_string(),
            },
        );
        Ok(format!("draft {} state={state} versions=1", id.trim()))
    }

    fn enqueue_outbox(&self, local_id: &str) -> Result<String, HostError> {
        let mut mem = self.lock();
        let row = mem
            .outbox
            .get_mut(local_id.trim())
            .ok_or_else(|| HostError::new(format!("no local post {local_id}")))?;
        row.status = "queued".into();
        row.keep = false;
        let action = if row.thread.is_empty() { "new_post" } else { "reply" };
        Ok(format!(
            "outbox Q-{} status=queued action={action}",
            local_id.trim()
        ))
    }

    fn cancel_outbox(&self, id: &str) -> Result<String, HostError> {
        let key = id.trim().trim_start_matches("Q-");
        let mut mem = self.lock();
        let row = mem
            .outbox
            .get_mut(key)
            .ok_or_else(|| HostError::new(format!("no outbox item {id}")))?;
        if row.status == "sent" {
            return Err(HostError::new(format!("outbox {id} is already sent")));
        }
        row.status = "cancelled".into();
        Ok(format!("outbox Q-{key} status=cancelled"))
    }

    fn export(&self, path: &str, request: &ExportRequest) -> Result<(), HostError> {
        let mem = self.lock();
        let posts: Vec<&ItemRec> = mem
            .items
            .values()
            .filter(|item| {
                request.post_ids.is_empty() || request.post_ids.iter().any(|id| id == &item.source_id.native)
            })
            .collect();
        if !request.post_ids.is_empty() && posts.is_empty() {
            return Err(HostError::new("none of the posts is in the database"));
        }
        let mut body = String::from("{\"posts\":[");
        for (i, item) in posts.iter().enumerate() {
            if i > 0 {
                body.push(',');
            }
            body.push_str(&format!(
                "{{\"post_id\":\"{}\",\"content\":\"{}\"}}",
                json_escape(&item.source_id.native),
                json_escape(&item.body)
            ));
        }
        body.push(']');
        if request.profile_images {
            body.push_str(",\"profiles\":[");
            let mut nicks: Vec<&str> = mem
                .items
                .values()
                .map(|item| item.nick.as_str())
                .filter(|nick| !nick.is_empty())
                .collect();
            nicks.sort_unstable();
            nicks.dedup();
            for (i, nick) in nicks.iter().enumerate() {
                if i > 0 {
                    body.push(',');
                }
                body.push_str(&format!("{{\"nick\":\"{}\"}}", json_escape(nick)));
            }
            body.push(']');
        }
        body.push('}');
        if request.format == ExportFormat::Html {
            body = format!("<article>{body}</article>");
        }
        drop(mem);
        if let Some(parent) = std::path::Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|err| HostError::new(err.to_string()))?;
        }
        if request.compress {
            write_tar_gz(path, &body).map_err(|err| HostError::new(err.to_string()))?;
        } else {
            std::fs::write(path, body).map_err(|err| HostError::new(err.to_string()))?;
        }
        Ok(())
    }

    fn sync_bookmarks(
        &self,
        service: &str,
        site: &[SiteBookmark],
        complete: bool,
    ) -> Result<BookmarkSync, HostError> {
        if service.is_empty() {
            return Err(HostError::new("the service id is empty"));
        }
        let mut mem = self.lock();
        let mut site_posts = Vec::new();
        for row in site {
            let post = if row.post_id.is_empty() {
                row.site_id.clone()
            } else {
                row.post_id.clone()
            };
            if post.is_empty() {
                return Err(HostError::new("a site bookmark needs an id"));
            }
            if !row.site_id.is_empty() && !row.post_id.is_empty() {
                mem.sites
                    .insert((service.to_string(), row.post_id.clone()), row.site_id.clone());
            }
            site_posts.push(post);
        }
        let local: Vec<(String, bool, bool)> = mem
            .bookmarks
            .iter()
            .map(|(id, mark)| (id.clone(), mark.deleted, mark.remote))
            .collect();
        let site_set: std::collections::BTreeSet<&str> = site_posts.iter().map(String::as_str).collect();
        let local_ids: std::collections::BTreeSet<&str> = local.iter().map(|(id, _, _)| id.as_str()).collect();
        let mut sync = BookmarkSync::default();
        for (id, deleted, remote) in &local {
            let on_site = site_set.contains(id.as_str());
            match (*deleted, on_site) {
                (false, true) => {
                    if let Some(mark) = mem.bookmarks.get_mut(id) {
                        mark.remote = true;
                    }
                }
                (false, false) => {
                    if *remote {
                        if complete {
                            mem.bookmarks.remove(id);
                            sync.dropped += 1;
                        }
                    } else {
                        sync.push_add.push(BookmarkChange {
                            site_id: mem
                                .sites
                                .get(&(service.to_string(), id.clone()))
                                .cloned()
                                .unwrap_or_default(),
                            post_id: id.clone(),
                        });
                    }
                }
                (true, true) => sync.push_remove.push(BookmarkChange {
                    site_id: site
                        .iter()
                        .find(|row| row.post_id == *id || row.site_id == *id)
                        .map(|row| row.site_id.clone())
                        .unwrap_or_default(),
                    post_id: id.clone(),
                }),
                (true, false) => {
                    mem.bookmarks.remove(id);
                    sync.dropped += 1;
                }
            }
        }
        for id in &site_posts {
            if !local_ids.contains(id.as_str()) {
                mem.bookmarks.insert(
                    id.clone(),
                    Mark {
                        remote: true,
                        deleted: false,
                    },
                );
                sync.pulled += 1;
            }
        }
        Ok(sync)
    }

    fn bookmark_pushed(
        &self,
        service: &str,
        post_id: &str,
        site_id: &str,
        added: bool,
    ) -> Result<(), HostError> {
        if service.is_empty() {
            return Err(HostError::new("the service id is empty"));
        }
        let mut mem = self.lock();
        if added {
            mem.bookmarks.insert(
                post_id.to_string(),
                Mark {
                    remote: true,
                    deleted: false,
                },
            );
            if !site_id.is_empty() {
                mem.sites
                    .insert((service.to_string(), post_id.to_string()), site_id.to_string());
            }
        } else {
            mem.bookmarks.remove(post_id);
            mem.sites.remove(&(service.to_string(), post_id.to_string()));
        }
        Ok(())
    }

    fn import_json(&self, path: &str) -> Result<usize, HostError> {
        let text = std::fs::read_to_string(path).map_err(|err| HostError::new(err.to_string()))?;
        if text.is_empty() {
            return Ok(0);
        }
        Ok(1)
    }

    fn preview_rules(&self) -> Result<String, HostError> {
        Ok(String::new())
    }

    fn add_research(&self, project: &str, item_id: &str, note: &str) -> Result<(), HostError> {
        self.lock()
            .research
            .push(format!("{project} {item_id} {note}"));
        Ok(())
    }

    fn remove_nick(&self, nick: &str, _keep_replies: bool, commit: bool) -> Result<String, HostError> {
        let verb = if commit { "removed" } else { "would remove" };
        let mut mem = self.lock();
        let n = mem.items.values().filter(|item| item.nick == nick).count();
        if commit {
            mem.items.retain(|_, item| item.nick != nick);
        }
        Ok(format!("{verb} {n}"))
    }

    fn push_notice(&self, notice: &NoticeRec) -> Result<i64, HostError> {
        let mut mem = self.lock();
        mem.notices.push(notice.clone());
        Ok(mem.notices.len() as i64)
    }

    fn pref(&self, key: &str) -> Result<Option<String>, HostError> {
        Ok(self.lock().prefs.get(key).cloned())
    }

    fn set_pref(&self, key: &str, value: &str) -> Result<(), HostError> {
        if key == "base_url" {
            return Err(HostError::new(
                "base URL comes from the ACP_BASE_URL environment variable",
            ));
        }
        self.lock().prefs.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn argv0(&self) -> String {
        "acp-red-star".into()
    }

    fn http_get(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
    ) -> Result<HttpResponse, HostError> {
        self.http(service, "GET", url, headers, &[])
    }

    fn http_post(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
        body: &[u8],
        _content_type: &str,
    ) -> Result<HttpResponse, HostError> {
        self.http(service, "POST", url, headers, body)
    }

    fn set_cookie(&self, service: &str, url: &str, name: &str, value: &str) -> Result<(), HostError> {
        if service.is_empty() {
            return Err(HostError::new("the service id is empty"));
        }
        let mut mem = self.lock();
        let slot = mem.cookies.entry(service.to_string()).or_default();
        if let Some(existing) = slot.iter_mut().find(|cookie| cookie.name == name && cookie.url == url) {
            existing.value = value.to_string();
        } else {
            slot.push(CookieRec {
                name: name.to_string(),
                value: value.to_string(),
                url: url.to_string(),
            });
        }
        Ok(())
    }

    fn cookies(&self, service: &str) -> Result<Vec<CookieRec>, HostError> {
        Ok(self.lock().cookies.get(service).cloned().unwrap_or_default())
    }

    fn clear_cookies(&self, service: &str) -> Result<(), HostError> {
        self.lock().cookies.remove(service);
        Ok(())
    }
}

impl MemHost {
    fn http(
        &self,
        service: &str,
        method: &str,
        url: &str,
        headers: &[HttpHeader],
        body: &[u8],
    ) -> Result<HttpResponse, HostError> {
        if service.is_empty() {
            return Err(HostError::new("the service id is empty"));
        }
        if url.is_empty() || !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(HostError::new("the URL must be http or https"));
        }
        for header in headers {
            if header.name.eq_ignore_ascii_case("cookie") {
                return Err(HostError::new("set cookies with set_cookie"));
            }
        }
        let mut mem = self.lock();
        let mut line = format!("{method} {url}");
        for header in headers {
            line.push_str(&format!("\n{}: {}", header.name, header.value));
        }
        let cookie_line = cookie_header(mem.cookies.get(service), url);
        if !cookie_line.is_empty() {
            line.push_str(&format!("\nCookie: {cookie_line}"));
        }
        if !body.is_empty() {
            line.push_str(&format!("\n{}", String::from_utf8_lossy(body)));
        }
        mem.http.last = line;
        let scripted = mem.http.headers.clone();
        let mut set_cookie_names = Vec::new();
        for header in &scripted {
            if header.name.eq_ignore_ascii_case("set-cookie") {
                if let Some((name, value)) = cookie_pair(&header.value) {
                    set_cookie_names.push(name.clone());
                    merge_cookie(
                        mem.cookies.entry(service.to_string()).or_default(),
                        CookieRec {
                            name,
                            value,
                            url: url.to_string(),
                        },
                    );
                }
            }
        }
        let final_url = if mem.http.url.is_empty() {
            url.to_string()
        } else {
            mem.http.url.clone()
        };
        Ok(HttpResponse {
            status: mem.http.status,
            body: mem.http.body.clone(),
            content_type: mem.http.content_type.clone(),
            url: final_url,
            headers: scripted,
            set_cookie_names,
        })
    }
}

fn cookie_pair(text: &str) -> Option<(String, String)> {
    let pair = text.split(';').next()?.trim();
    let (name, value) = pair.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), value.trim().to_string()))
}

fn merge_cookie(slot: &mut Vec<CookieRec>, cookie: CookieRec) {
    if let Some(existing) = slot
        .iter_mut()
        .find(|row| row.name == cookie.name && row.url == cookie.url)
    {
        existing.value = cookie.value;
    } else {
        slot.push(cookie);
    }
}

fn cookie_header(cookies: Option<&Vec<CookieRec>>, url: &str) -> String {
    let Some(cookies) = cookies else {
        return String::new();
    };
    cookies
        .iter()
        .filter(|cookie| same_origin(&cookie.url, url))
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

fn same_origin(left: &str, right: &str) -> bool {
    origin(left).is_some() && origin(left) == origin(right)
}

fn origin(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    rest.split('/').next()
}

fn segment_item(segment: &crate::records::SegmentRec) -> ItemRec {
    let mut item = ItemRec::new(segment.source_id.clone(), segment.body.clone());
    item.parent = Some(segment.parent.clone());
    item.nick = segment.speaker.clone();
    item
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_tar_gz(path: &str, body: &str) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|err| err.to_string())?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(gz);
    let bytes = body.as_bytes();
    let mut header = tar::Header::new_gnu();
    header.set_path("export.txt").map_err(|err| err.to_string())?;
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append(&header, std::io::Cursor::new(bytes))
        .map_err(|err| err.to_string())?;
    builder
        .into_inner()
        .map_err(|err| err.to_string())?
        .finish()
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::ServiceId;

    #[test]
    fn a_page_outbox_export_and_bookmark_sync_stay_in_memory() {
        let host = MemHost::new("/tmp/mem-host");
        let id = SourceId::new(ServiceId::new("redstar"), "post-1");
        let mut item = ItemRec::new(id.clone(), "hello");
        item.nick = "ada".into();
        let mut reply = ItemRec::new(SourceId::new(ServiceId::new("redstar"), "reply-1"), "pong");
        reply.parent = Some(id.clone());
        let page = Page {
            items: vec![item, reply],
            ..Page::empty()
        };
        assert_eq!(host.save_page(&page).unwrap(), 2);
        let known = host.known_ids().unwrap();
        assert!(known.ids.iter().any(|id| id == "post-1"));
        assert_eq!(
            known.reply_counts.iter().find(|row| row.thread_id == "post-1").map(|row| row.replies),
            Some(1)
        );

        host.save_outbox("d1", "Title", "body", "", false).unwrap();
        host.enqueue_outbox("d1").unwrap();
        assert!(host.list_outbox().unwrap()[0].contains("status=queued"));
        host.cancel_outbox("Q-d1").unwrap();
        assert!(host.list_outbox().unwrap()[0].contains("status=cancelled"));

        host.add_bookmark("local-1").unwrap();
        let sync = host
            .sync_bookmarks(
                "redstar",
                &[SiteBookmark::new("site-9", "post-9")],
                true,
            )
            .unwrap();
        assert_eq!(sync.pulled, 1);
        assert_eq!(sync.push_add.len(), 1);
        assert_eq!(sync.push_add[0].post_id, "local-1");
        host.bookmark_pushed("redstar", "local-1", "site-1", true).unwrap();
        assert!(host.list_bookmarks().unwrap().iter().any(|id| id == "local-1"));

        let path = std::env::temp_dir().join(format!("mem-export-{}.tar.gz", std::process::id()));
        let mut request = ExportRequest::new(ExportFormat::Json);
        request.compress = true;
        request.profile_images = true;
        request.post_ids = vec!["post-1".into()];
        host.export(&path.display().to_string(), &request).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let _ = std::fs::remove_file(&path);

        host.script_http(201, b"ok".to_vec(), "text/plain");
        host.set_cookie("redstar", "http://127.0.0.1/", "sid", "abc").unwrap();
        let response = host
            .http_get("redstar", "http://127.0.0.1/", &[HttpHeader::new("X-Test", "yes")])
            .unwrap();
        assert_eq!(response.status, 201);
        assert!(host.last_http().contains("X-Test: yes"));
        assert!(host.http_get("redstar", "http://127.0.0.1/", &[HttpHeader::new("Cookie", "no")]).is_err());
    }

    #[test]
    fn login_cookie_survives_set_cookie_and_the_next_request() {
        let host = MemHost::new("/tmp/mem-host");
        host.script_http(302, Vec::<u8>::new(), "text/plain");
        host.script_final_url("http://127.0.0.1/home");
        host.script_headers([
            HttpHeader::new("Location", "http://127.0.0.1/home"),
            HttpHeader::new("Set-Cookie", "session=s3cret; Path=/; HttpOnly"),
        ]);
        let response = host
            .http_post(
                "redstar",
                "http://127.0.0.1/login",
                &[],
                b"email=ada",
                "application/x-www-form-urlencoded",
            )
            .unwrap();
        assert!(response.url.ends_with("/home"), "{}", response.url);
        assert!(response.headers.iter().any(|header| header.name.eq_ignore_ascii_case("location")));
        assert_eq!(response.set_cookie_names, vec!["session".to_string()]);
        assert!(!format!("{response:?}").contains("s3cret"), "{response:?}");
        let stored = host.cookies("redstar").unwrap();
        assert!(stored.iter().any(|cookie| cookie.name == "session" && cookie.value == "s3cret"));
        assert!(!format!("{stored:?}").contains("s3cret"), "{stored:?}");
        host.set_cookie("redstar", "http://127.0.0.1/", "theme", "dark").unwrap();
        let stored = host.cookies("redstar").unwrap();
        assert!(stored.iter().any(|cookie| cookie.name == "session" && cookie.value == "s3cret"));
        assert!(stored.iter().any(|cookie| cookie.name == "theme" && cookie.value == "dark"));
        host.script_http(200, b"ok".to_vec(), "text/plain");
        let _ = host.http_get("redstar", "http://127.0.0.1/home", &[]).unwrap();
        let last = host.last_http();
        assert!(last.contains("session=s3cret"), "{last}");
        assert!(last.contains("theme=dark"), "{last}");
    }

    #[test]
    fn bookmark_post_marks_and_clears_in_memory() {
        let host = MemHost::new("/tmp/mem-host");
        assert!(host.bookmark_post("", "post-1", true).is_err());
        host.bookmark_post("redstar", "post-1", true).unwrap();
        assert!(host.list_bookmarks().unwrap().iter().any(|id| id == "post-1"));
        host.bookmark_post("redstar", "post-1", false).unwrap();
        assert!(!host.list_bookmarks().unwrap().iter().any(|id| id == "post-1"));
    }
}
