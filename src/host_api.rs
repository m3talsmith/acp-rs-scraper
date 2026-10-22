//! The core's [`acp_rs_plugin::Host`].
//!
//! A service reaches storage, HTTP, configuration, secrets, bookmarks, the
//! outbox, export, import, rules, research, maintenance, notices, and
//! preferences through this type. It does not open SQLCipher and it does not
//! link the node crate. HTTP here is a plain client with one cookie jar per
//! service. It does not impersonate a browser. The LiveView socket client
//! stays in the service.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use acp_rs_plugin::{
    AccountRec, AttachmentRec, BodyState, BookmarkChange, BookmarkSync, CookieRec, ExportFormat,
    ExportRequest, Host, HostError, HttpHeader, HttpResponse, ItemRec, KnownIds, NoticeRec, Page,
    SiteBookmark, SourceId, ThreadReplies,
};
use rusqlite::{Connection, params};

use crate::config::{ATTACH_REL_PATH, DB_REL_PATH, PREFS_REL_PATH};
use crate::prefs::{Prefs, UiPrefs};
use crate::stored::Post;

const SECRET_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS host_secrets (
    field TEXT PRIMARY KEY,
    material BLOB NOT NULL
);";

const BOOKMARK_SITE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS bookmark_sites (
    service TEXT NOT NULL,
    post_id TEXT NOT NULL,
    site_id TEXT NOT NULL,
    PRIMARY KEY (service, post_id)
);";

/// Binary name. [`CoreHost::argv0`] returns this and does not spawn a process.
pub const ARGV0: &str = "acp-red-star";

struct ServiceHttp {
    jar: Arc<ServiceJar>,
}

/// Core implementation of [`Host`].
pub struct CoreHost {
    http: Mutex<HashMap<String, ServiceHttp>>,
    home_override: Option<PathBuf>,
}

impl CoreHost {
    /// The process host. The database is the one under [`crate::config::home`].
    pub fn shared() -> Arc<dyn Host> {
        static HOST: OnceLock<Arc<CoreHost>> = OnceLock::new();
        HOST.get_or_init(|| {
            Arc::new(CoreHost {
                http: Mutex::new(HashMap::new()),
                home_override: None,
            })
        })
        .clone()
    }

    /// A host whose files live under `home`. Tests use this so they do not read the process home.
    pub fn for_home(home: impl Into<PathBuf>) -> Arc<dyn Host> {
        Arc::new(CoreHost {
            http: Mutex::new(HashMap::new()),
            home_override: Some(home.into()),
        })
    }

    fn home_path(&self) -> Result<PathBuf, HostError> {
        if let Some(home) = &self.home_override {
            return Ok(home.clone());
        }
        crate::config::home().map_err(host_err)
    }

    fn db_path(&self) -> Result<PathBuf, HostError> {
        Ok(self.home_path()?.join(DB_REL_PATH))
    }

    fn open(&self) -> Result<Connection, HostError> {
        crate::storage::db::open_db(&self.db_path()?).map_err(host_err)
    }

    fn prefs_path(&self) -> Result<PathBuf, HostError> {
        Ok(self.home_path()?.join(PREFS_REL_PATH))
    }

    fn load_prefs(&self) -> Result<Prefs, HostError> {
        let path = self.prefs_path()?;
        if !path.is_file() {
            return Ok(Prefs::default());
        }
        let text = std::fs::read_to_string(&path).map_err(host_err)?;
        toml::from_str(&text).map_err(host_err)
    }

    fn write_prefs(&self, prefs: &Prefs) -> Result<(), HostError> {
        let path = self.prefs_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(host_err)?;
        }
        let text = toml::to_string(prefs).map_err(host_err)?;
        std::fs::write(path, text).map_err(host_err)
    }
}

impl Host for CoreHost {
    fn home(&self) -> Result<String, HostError> {
        Ok(self.home_path()?.display().to_string())
    }

    fn database_path(&self) -> Result<String, HostError> {
        Ok(self.db_path()?.display().to_string())
    }

    fn base_url(&self) -> Result<String, HostError> {
        Ok(crate::config::base_url().to_string())
    }

    fn email(&self) -> Result<String, HostError> {
        Ok(self.pref("email")?.unwrap_or_default())
    }

    fn save_page(&self, page: &Page) -> Result<usize, HostError> {
        let mut posts = Vec::new();
        for item in &page.items {
            posts.push(item_to_post(item)?);
        }
        for segment in &page.segments {
            if segment.source_id.native.is_empty() {
                return Err(HostError::new("a segment needs a native id"));
            }
            posts.push(Post {
                post_id: segment.source_id.native.clone(),
                parent_post_id: segment.parent.native.clone(),
                in_reply_to_id: String::new(),
                nick: segment.speaker.clone(),
                title: String::new(),
                content: segment.body.clone(),
                posted_at: String::new(),
                media: Vec::new(),
                truncated: false,
                body_json: String::new(),
                score: None,
            });
        }
        let mut db = self.open()?;
        let n = if posts.is_empty() {
            0
        } else {
            crate::storage::db::save_posts(&mut db, &posts).map_err(host_err)?
        };
        for item in &page.items {
            db.execute(
                "UPDATE posts SET body_state = ?1 WHERE post_id = ?2",
                params![item.body_state.as_str(), item.source_id.native],
            )
            .map_err(host_err)?;
        }
        let path = self.db_path()?;
        let mut attachments = Vec::new();
        for item in &page.items {
            for row in &item.attachments {
                if row.url.is_empty() {
                    return Err(HostError::new("an attachment needs a url"));
                }
                attachments.push(crate::stored::Attachment {
                    post_id: item.source_id.native.clone(),
                    url: row.url.clone(),
                    kind: if row.kind.is_empty() {
                        "file".into()
                    } else {
                        row.kind.clone()
                    },
                    local_path: PathBuf::from(&row.local_path),
                    mime_type: row.mime.clone(),
                    on_site: row.on_site,
                });
            }
        }
        if !attachments.is_empty() {
            crate::storage::db::save_attachments(&path, &attachments).map_err(host_err)?;
        }
        for account in &page.accounts {
            crate::storage::db::save_profile(&path, &account_to_profile(account)?)
                .map_err(host_err)?;
        }
        Ok(n)
    }

    fn load_item(&self, id: &SourceId) -> Result<Option<ItemRec>, HostError> {
        if id.native.is_empty() {
            return Ok(None);
        }
        let (posts, _, missing) =
            crate::storage::db::posts_by_ids(&self.db_path()?, &[id.native.clone()])
                .map_err(host_err)?;
        if missing.iter().any(|m| m == &id.native) || posts.is_empty() {
            return Ok(None);
        }
        let mut item = post_to_item(&posts[0]);
        let db = self.open()?;
        let state: String = db
            .query_row(
                "SELECT body_state FROM posts WHERE post_id = ?1",
                params![id.native],
                |row| row.get(0),
            )
            .map_err(host_err)?;
        item.body_state = BodyState::parse(&state);
        item.truncated = item.body_state == BodyState::Truncated;
        item.attachments = load_attachment_rows(&db, &id.native)?;
        Ok(Some(item))
    }

    fn known_ids(&self) -> Result<KnownIds, HostError> {
        let path = self.db_path()?;
        let mut ids: Vec<String> = crate::storage::db::db_known_ids(&path)
            .map_err(host_err)?
            .into_iter()
            .collect();
        ids.sort();
        let mut reply_counts: Vec<ThreadReplies> = crate::storage::db::db_reply_counts(&path)
            .map_err(host_err)?
            .into_iter()
            .map(|(thread_id, replies)| ThreadReplies {
                thread_id,
                replies: replies as u64,
            })
            .collect();
        reply_counts.sort_by(|a, b| a.thread_id.cmp(&b.thread_id));
        Ok(KnownIds { ids, reply_counts })
    }

    fn store_secret(&self, field: &str, material: &[u8]) -> Result<(), HostError> {
        if field.is_empty() {
            return Err(HostError::new("a secret needs a field name"));
        }
        let db = self.open()?;
        db.execute_batch(SECRET_SCHEMA).map_err(host_err)?;
        db.execute(
            "INSERT INTO host_secrets (field, material) VALUES (?1, ?2)
             ON CONFLICT(field) DO UPDATE SET material = excluded.material",
            params![field, material],
        )
        .map_err(host_err)?;
        Ok(())
    }

    fn load_secret(&self, field: &str) -> Result<Option<Vec<u8>>, HostError> {
        let db = self.open()?;
        db.execute_batch(SECRET_SCHEMA).map_err(host_err)?;
        match db.query_row(
            "SELECT material FROM host_secrets WHERE field = ?1",
            params![field],
            |r| r.get::<_, Vec<u8>>(0),
        ) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(host_err(err)),
        }
    }

    fn list_bookmarks(&self) -> Result<Vec<String>, HostError> {
        let db = crate::bookmarks::bookmarks_conn(&self.db_path()?).map_err(host_err)?;
        let rows = crate::bookmarks::load_bookmarks(&db).map_err(host_err)?;
        Ok(rows
            .into_iter()
            .filter(|row| !row.deleted)
            .map(|row| row.post_id)
            .collect())
    }

    fn add_bookmark(&self, id: &str) -> Result<(), HostError> {
        let db = crate::bookmarks::bookmarks_conn(&self.db_path()?).map_err(host_err)?;
        let now = chrono::Local::now().to_rfc3339();
        crate::bookmarks::add_bookmarks(&db, &[id.to_string()], &now).map_err(host_err)?;
        Ok(())
    }

    fn remove_bookmark(&self, id: &str) -> Result<(), HostError> {
        let db = crate::bookmarks::bookmarks_conn(&self.db_path()?).map_err(host_err)?;
        crate::bookmarks::remove_bookmarks(&db, &[id.to_string()]).map_err(host_err)?;
        Ok(())
    }

    fn list_outbox(&self) -> Result<Vec<String>, HostError> {
        let db = self.open()?;
        crate::outbox::list_outbox(&db).map_err(host_err)
    }

    fn save_outbox(
        &self,
        id: &str,
        title: &str,
        body: &str,
        thread: &str,
        keep: bool,
    ) -> Result<String, HostError> {
        let mut db = self.open()?;
        crate::outbox::save_local(&mut db, id, title, body, thread, keep).map_err(host_err)
    }

    fn enqueue_outbox(&self, local_id: &str) -> Result<String, HostError> {
        let mut db = self.open()?;
        crate::outbox::enqueue(&mut db, local_id).map_err(host_err)
    }

    fn cancel_outbox(&self, id: &str) -> Result<String, HostError> {
        let mut db = self.open()?;
        crate::outbox::cancel(&mut db, id).map_err(host_err)
    }

    fn export(&self, path: &str, request: &ExportRequest) -> Result<(), HostError> {
        let db = self.open()?;
        crate::storage::db::ensure_posts_schema(&db).map_err(host_err)?;
        let db_path = self.db_path()?;
        let (posts, attachments) = if request.post_ids.is_empty() {
            let posts = crate::rules::load_posts(&db).map_err(host_err)?;
            let attachments = attachments_for_posts(&db, &posts)?;
            (posts, attachments)
        } else {
            let (posts, attachments, missing) =
                crate::storage::db::posts_by_ids(&db_path, &request.post_ids).map_err(host_err)?;
            if posts.is_empty() {
                return Err(HostError::new("none of the posts is in the database"));
            }
            let _ = missing;
            (posts, attachments)
        };
        let nicks: Vec<String> = posts.iter().map(|post| post.nick.clone()).filter(|nick| !nick.is_empty()).collect();
        let (profiles, hits) = if request.profile_images {
            crate::storage::db::load_profiles_mentions(&db_path, &nicks).map_err(host_err)?
        } else {
            (Vec::new(), Vec::new())
        };
        write_export(path, request, &posts, &attachments, &profiles, &hits)
    }

    fn sync_bookmarks(
        &self,
        service: &str,
        site: &[SiteBookmark],
        complete: bool,
    ) -> Result<BookmarkSync, HostError> {
        check_service(service)?;
        let db = crate::bookmarks::bookmarks_conn(&self.db_path()?).map_err(host_err)?;
        db.execute_batch(BOOKMARK_SITE_SCHEMA).map_err(host_err)?;
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
                db.execute(
                    "INSERT INTO bookmark_sites (service, post_id, site_id) VALUES (?1, ?2, ?3)
                     ON CONFLICT(service, post_id) DO UPDATE SET site_id = excluded.site_id",
                    params![service, row.post_id, row.site_id],
                )
                .map_err(host_err)?;
            }
            site_posts.push(post);
        }
        let local = crate::bookmarks::load_bookmarks(&db).map_err(host_err)?;
        let plan = plan_bookmark_sync(&local, &site_posts, complete);
        let now = chrono::Local::now().to_rfc3339();
        for id in &plan.insert_remote {
            db.execute(
                "INSERT OR IGNORE INTO bookmarks (post_id, created_at, remote) VALUES (?1, ?2, 1)",
                params![id, now],
            )
            .map_err(host_err)?;
        }
        for id in &plan.set_remote {
            db.execute("UPDATE bookmarks SET remote = 1 WHERE post_id = ?1", params![id])
                .map_err(host_err)?;
        }
        for id in &plan.delete_rows {
            db.execute("DELETE FROM bookmarks WHERE post_id = ?1", params![id])
                .map_err(host_err)?;
            db.execute(
                "DELETE FROM bookmark_sites WHERE service = ?1 AND post_id = ?2",
                params![service, id],
            )
            .map_err(host_err)?;
        }
        let mut sync = BookmarkSync {
            pulled: plan.insert_remote.len() as u64,
            dropped: plan.delete_rows.len() as u64,
            ..BookmarkSync::default()
        };
        for id in plan.push_add {
            sync.push_add.push(BookmarkChange {
                site_id: mapped_site_id(&db, service, &id)?,
                post_id: id,
            });
        }
        for id in plan.push_remove {
            let site_id = site
                .iter()
                .find(|row| row.post_id == id || row.site_id == id)
                .map(|row| row.site_id.clone())
                .filter(|id| !id.is_empty())
                .unwrap_or(mapped_site_id(&db, service, &id)?);
            sync.push_remove.push(BookmarkChange { site_id, post_id: id });
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
        check_service(service)?;
        let db = crate::bookmarks::bookmarks_conn(&self.db_path()?).map_err(host_err)?;
        db.execute_batch(BOOKMARK_SITE_SCHEMA).map_err(host_err)?;
        if added {
            db.execute("UPDATE bookmarks SET remote = 1, deleted = 0 WHERE post_id = ?1", params![post_id])
                .map_err(host_err)?;
            if !site_id.is_empty() {
                db.execute(
                    "INSERT INTO bookmark_sites (service, post_id, site_id) VALUES (?1, ?2, ?3)
                     ON CONFLICT(service, post_id) DO UPDATE SET site_id = excluded.site_id",
                    params![service, post_id, site_id],
                )
                .map_err(host_err)?;
            }
        } else {
            db.execute("DELETE FROM bookmarks WHERE post_id = ?1", params![post_id])
                .map_err(host_err)?;
            db.execute(
                "DELETE FROM bookmark_sites WHERE service = ?1 AND post_id = ?2",
                params![service, post_id],
            )
            .map_err(host_err)?;
        }
        Ok(())
    }

    fn import_json(&self, path: &str) -> Result<usize, HostError> {
        let home = self.home_path()?;
        let att = home.join(ATTACH_REL_PATH);
        let tmp = home.join(".acp/tmp");
        std::fs::create_dir_all(&tmp).map_err(host_err)?;
        let (posts, _, profiles, _) =
            crate::import::import(Path::new(path), None, &att, &tmp).map_err(host_err)?;
        let mut db = self.open()?;
        let n = crate::storage::db::save_posts(&mut db, &posts).map_err(host_err)?;
        let db_path = self.db_path()?;
        for profile in &profiles {
            crate::storage::db::save_profile(&db_path, profile).map_err(host_err)?;
        }
        Ok(n)
    }

    fn preview_rules(&self) -> Result<String, HostError> {
        let firings = crate::rules::preview(&self.db_path()?).map_err(host_err)?;
        Ok(firings
            .into_iter()
            .map(|firing| format!("{} {} {}", firing.rule_name, firing.outcome, firing.detail))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn add_research(&self, project: &str, item_id: &str, note: &str) -> Result<(), HostError> {
        let db = crate::research::research_conn(&self.db_path()?).map_err(host_err)?;
        let id = match crate::research::get_project(&db, project).map_err(host_err)? {
            Some(found) => found.id,
            None => crate::research::create_project(&db, project, "").map_err(host_err)?,
        };
        let item = crate::research::NewItem {
            kind: Some(crate::research::ItemKind::Post),
            r#ref: item_id.to_string(),
            title: item_id.to_string(),
            note: note.to_string(),
            ..crate::research::NewItem::default()
        };
        crate::research::add_item(&db, id, &item).map_err(host_err)?;
        Ok(())
    }

    fn remove_nick(
        &self,
        nick: &str,
        keep_replies: bool,
        commit: bool,
    ) -> Result<String, HostError> {
        let db = self.db_path()?;
        let att = self.home_path()?.join(ATTACH_REL_PATH);
        let report = crate::maintenance::remove_nicks(
            &db,
            &att,
            &[nick.to_string()],
            keep_replies,
            commit,
        )
        .map_err(host_err)?;
        let verb = if commit { "removed" } else { "would remove" };
        Ok(report.summary(verb))
    }

    fn push_notice(&self, notice: &NoticeRec) -> Result<i64, HostError> {
        let db = self.open()?;
        let row = crate::storage::notices::NoticeRow {
            id: 0,
            created_at: chrono::Local::now().to_rfc3339(),
            kind: notice.kind.clone(),
            title: notice.title.clone(),
            body: notice.body.clone(),
        };
        crate::storage::notices::insert(&db, &row).map_err(host_err)
    }

    fn pref(&self, key: &str) -> Result<Option<String>, HostError> {
        match key {
            "base_url" => Ok(Some(crate::config::base_url().to_string())),
            "email" => Ok(self.load_prefs()?.email.filter(|email| !email.is_empty())),
            "theme" => Ok(self.load_prefs()?.ui.and_then(|ui| ui.theme)),
            "brand" => Ok(self.load_prefs()?.ui.and_then(|ui| ui.brand)),
            _ => Err(HostError::new(format!("unknown preference {key}"))),
        }
    }

    fn set_pref(&self, key: &str, value: &str) -> Result<(), HostError> {
        if key == "base_url" {
            return Err(HostError::new(
                "base URL comes from the ACP_BASE_URL environment variable",
            ));
        }
        let mut prefs = self.load_prefs()?;
        match key {
            "email" => prefs.email = Some(value.to_string()),
            "theme" => {
                prefs.ui.get_or_insert_with(UiPrefs::default).theme = Some(value.to_string());
            }
            "brand" => {
                prefs.ui.get_or_insert_with(UiPrefs::default).brand = Some(value.to_string());
            }
            _ => return Err(HostError::new(format!("unknown preference {key}"))),
        }
        self.write_prefs(&prefs)
    }

    fn argv0(&self) -> String {
        ARGV0.to_string()
    }

    fn http_get(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
    ) -> Result<HttpResponse, HostError> {
        self.http(service, "GET", url, headers, None, "")
    }

    fn http_post(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
        body: &[u8],
        content_type: &str,
    ) -> Result<HttpResponse, HostError> {
        self.http(service, "POST", url, headers, Some(body.to_vec()), content_type)
    }

    fn set_cookie(&self, service: &str, url: &str, name: &str, value: &str) -> Result<(), HostError> {
        check_service(service)?;
        if url.is_empty() || (!url.starts_with("https://") && !url.starts_with("http://")) {
            return Err(HostError::new("the cookie URL must be http or https"));
        }
        if name.is_empty() || name.contains([';', ' ', '=']) {
            return Err(HostError::new("the cookie name is empty or has a separator"));
        }
        let mut http = self.http.lock().unwrap_or_else(|err| err.into_inner());
        let slot = http.entry(service.to_string()).or_insert_with(ServiceHttp::empty);
        // Merge into the existing jar. A cookie the server already stored stays.
        // The value is not written to a log.
        slot.jar.store(url, name, value).map_err(host_err)?;
        Ok(())
    }

    fn cookies(&self, service: &str) -> Result<Vec<CookieRec>, HostError> {
        check_service(service)?;
        let http = self.http.lock().unwrap_or_else(|err| err.into_inner());
        Ok(http.get(service).map(|slot| slot.jar.records()).unwrap_or_default())
    }

    fn clear_cookies(&self, service: &str) -> Result<(), HostError> {
        check_service(service)?;
        self.http.lock().unwrap_or_else(|err| err.into_inner()).remove(service);
        Ok(())
    }
}

impl CoreHost {
    fn http(
        &self,
        service: &str,
        method: &str,
        url: &str,
        headers: &[HttpHeader],
        body: Option<Vec<u8>>,
        content_type: &str,
    ) -> Result<HttpResponse, HostError> {
        check_service(service)?;
        if url.is_empty() {
            return Err(HostError::new("the URL is empty"));
        }
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(HostError::new("the URL must be http or https"));
        }
        for header in headers {
            if header.name.eq_ignore_ascii_case("cookie") {
                return Err(HostError::new("set cookies with set_cookie"));
            }
            if header.name.is_empty() {
                return Err(HostError::new("a header needs a name"));
            }
        }
        let jar = {
            let mut http = self.http.lock().unwrap_or_else(|err| err.into_inner());
            let slot = http.entry(service.to_string()).or_insert_with(ServiceHttp::empty);
            Arc::clone(&slot.jar)
        };
        let method = method.to_string();
        let url = url.to_string();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        let content_type = if content_type.is_empty() {
            "application/octet-stream".to_string()
        } else {
            content_type.to_string()
        };
        let locations = Arc::new(Mutex::new(Vec::<String>::new()));
        let locations_for_worker = Arc::clone(&locations);
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(host_err)?;
            runtime.block_on(async move {
                let followed = Arc::clone(&locations_for_worker);
                let policy = reqwest::redirect::Policy::custom(move |attempt| {
                    followed
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .push(attempt.url().to_string());
                    if attempt.previous().len() > 10 {
                        attempt.error("too many redirects")
                    } else {
                        attempt.follow()
                    }
                });
                let observed = Arc::new(ObservedJar::new(Arc::clone(&jar)));
                let names = Arc::clone(&observed);
                let client = reqwest::Client::builder()
                    .redirect(policy)
                    .cookie_provider(observed)
                    .build()
                    .map_err(host_err)?;
                let mut request = match method.as_str() {
                    "POST" => client
                        .post(&url)
                        .timeout(std::time::Duration::from_secs(30))
                        .header(reqwest::header::CONTENT_TYPE, content_type)
                        .body(body.unwrap_or_default()),
                    _ => client.get(&url).timeout(std::time::Duration::from_secs(30)),
                };
                for (name, value) in headers {
                    request = request.header(name, value);
                }
                let response = request.send().await.map_err(host_err)?;
                let status = response.status().as_u16();
                let final_url = response.url().to_string();
                let response_headers = header_list(response.headers());
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = response.bytes().await.map_err(host_err)?.to_vec();
                let set_cookie_names = names.take_names();
                Ok(HttpResponse {
                    status,
                    body,
                    content_type,
                    url: final_url,
                    headers: response_headers,
                    set_cookie_names,
                })
            })
        });
        let mut response = match worker.join() {
            Ok(result) => result?,
            Err(_) => return Err(HostError::new("the HTTP worker stopped")),
        };
        let followed = locations.lock().unwrap_or_else(|err| err.into_inner()).clone();
        for location in followed {
            let already = response.headers.iter().any(|header| {
                header.name.eq_ignore_ascii_case("location") && header.value == location
            });
            if !already {
                response.headers.push(HttpHeader::new("location", location));
            }
        }
        Ok(response)
    }
}

/// One cookie jar per service. Records and the reqwest jar stay in step.
///
/// `cookies` reads the records. `set_cookie` merges into this jar. A response
/// `Set-Cookie` updates the same records. Nothing here replaces the jar, so a
/// later `set_cookie` cannot drop a session cookie the server stored.
struct ServiceJar {
    inner: reqwest::cookie::Jar,
    records: Mutex<Vec<CookieRec>>,
}

/// Cookie store for one call. Names set during the call stay on this value.
/// The shared [`ServiceJar`] keeps the cookies.
struct ObservedJar {
    inner: Arc<ServiceJar>,
    names: Mutex<Vec<String>>,
}

impl ServiceJar {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: reqwest::cookie::Jar::default(),
            records: Mutex::new(Vec::new()),
        })
    }

    fn records(&self) -> Vec<CookieRec> {
        self.records.lock().unwrap_or_else(|err| err.into_inner()).clone()
    }

    fn merge(&self, cookie: CookieRec) {
        let mut records = self.records.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(existing) = records
            .iter_mut()
            .find(|row| row.name == cookie.name && row.url == cookie.url)
        {
            existing.value = cookie.value;
        } else {
            records.push(cookie);
        }
    }

    /// Store `name`=`value` for `url` on the existing jar. The value is not logged.
    fn store(&self, url: &str, name: &str, value: &str) -> Result<(), String> {
        let parsed = reqwest::Url::parse(url).map_err(|err| err.to_string())?;
        self.inner
            .add_cookie_str(&format!("{name}={value}; Path=/"), &parsed);
        self.merge(CookieRec {
            name: name.to_string(),
            value: value.to_string(),
            url: url.to_string(),
        });
        Ok(())
    }
}

impl reqwest::cookie::CookieStore for ServiceJar {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &reqwest::header::HeaderValue>,
        url: &reqwest::Url,
    ) {
        let headers: Vec<reqwest::header::HeaderValue> = cookie_headers.cloned().collect();
        for header in &headers {
            if let Some((name, value)) = split_cookie_pair(header) {
                self.merge(CookieRec {
                    name,
                    value,
                    url: url.as_str().to_string(),
                });
            }
        }
        self.inner.set_cookies(&mut headers.iter(), url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<reqwest::header::HeaderValue> {
        reqwest::cookie::CookieStore::cookies(&self.inner, url)
    }
}

impl ObservedJar {
    fn new(inner: Arc<ServiceJar>) -> Self {
        Self {
            inner,
            names: Mutex::new(Vec::new()),
        }
    }

    fn take_names(&self) -> Vec<String> {
        let names = self.names.lock().unwrap_or_else(|err| err.into_inner());
        let mut out = Vec::new();
        for name in names.iter() {
            if !out.iter().any(|have: &String| have == name) {
                out.push(name.clone());
            }
        }
        out
    }
}

impl reqwest::cookie::CookieStore for ObservedJar {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &reqwest::header::HeaderValue>,
        url: &reqwest::Url,
    ) {
        let headers: Vec<reqwest::header::HeaderValue> = cookie_headers.cloned().collect();
        {
            let mut names = self.names.lock().unwrap_or_else(|err| err.into_inner());
            for header in &headers {
                if let Some((name, _)) = split_cookie_pair(header) {
                    names.push(name);
                }
            }
        }
        self.inner.set_cookies(&mut headers.iter(), url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<reqwest::header::HeaderValue> {
        self.inner.cookies(url)
    }
}

impl ServiceHttp {
    fn empty() -> Self {
        Self {
            jar: ServiceJar::new(),
        }
    }
}

fn header_list(headers: &reqwest::header::HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|(name, value)| {
            let text = value
                .to_str()
                .map(str::to_string)
                .unwrap_or_else(|_| String::from_utf8_lossy(value.as_bytes()).into_owned());
            HttpHeader::new(name.as_str(), text)
        })
        .collect()
}

/// Name and value from a `Set-Cookie` header. The attributes after `;` are dropped.
fn split_cookie_pair(header: &reqwest::header::HeaderValue) -> Option<(String, String)> {
    let text = header.to_str().ok()?;
    cookie_pair(text)
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

fn check_service(service: &str) -> Result<(), HostError> {
    if service.is_empty() {
        Err(HostError::new("the service id is empty"))
    } else {
        Ok(())
    }
}

fn load_attachment_rows(db: &rusqlite::Connection, post_id: &str) -> Result<Vec<AttachmentRec>, HostError> {
    crate::storage::db::ensure_attachments_schema(db).map_err(host_err)?;
    let mut statement = db
        .prepare(
            "SELECT url, local_path, mime_type, on_site FROM attachments WHERE post_id = ?1 ORDER BY id",
        )
        .map_err(host_err)?;
    let rows = statement
        .query_map(params![post_id], |row| {
            Ok(AttachmentRec {
                url: row.get(0)?,
                kind: "file".into(),
                mime: row.get(2)?,
                local_path: row.get(1)?,
                on_site: row.get::<_, i64>(3)? != 0,
            })
        })
        .map_err(host_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(host_err)
}

struct BookmarkPlan {
    push_add: Vec<String>,
    push_remove: Vec<String>,
    insert_remote: Vec<String>,
    set_remote: Vec<String>,
    delete_rows: Vec<String>,
}

/// Reconcile a site bookmark list with local rows. Local changes win.
fn plan_bookmark_sync(local: &[crate::bookmarks::Bookmark], site: &[String], site_complete: bool) -> BookmarkPlan {
    let site_set: std::collections::HashSet<&String> = site.iter().collect();
    let local_ids: std::collections::HashSet<&String> = local.iter().map(|row| &row.post_id).collect();
    let mut plan = BookmarkPlan {
        push_add: Vec::new(),
        push_remove: Vec::new(),
        insert_remote: Vec::new(),
        set_remote: Vec::new(),
        delete_rows: Vec::new(),
    };
    for row in local {
        let on_site = site_set.contains(&row.post_id);
        match (row.deleted, on_site) {
            (false, true) => {
                if !row.remote {
                    plan.set_remote.push(row.post_id.clone());
                }
            }
            (false, false) => {
                if row.remote {
                    if site_complete {
                        plan.delete_rows.push(row.post_id.clone());
                    }
                } else {
                    plan.push_add.push(row.post_id.clone());
                }
            }
            (true, true) => plan.push_remove.push(row.post_id.clone()),
            (true, false) => plan.delete_rows.push(row.post_id.clone()),
        }
    }
    for id in site {
        if !local_ids.contains(id) {
            plan.insert_remote.push(id.clone());
        }
    }
    plan
}

fn mapped_site_id(db: &rusqlite::Connection, service: &str, post_id: &str) -> Result<String, HostError> {
    match db.query_row(
        "SELECT site_id FROM bookmark_sites WHERE service = ?1 AND post_id = ?2",
        params![service, post_id],
        |row| row.get(0),
    ) {
        Ok(id) => Ok(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(String::new()),
        Err(err) => Err(host_err(err)),
    }
}

fn attachments_for_posts(db: &rusqlite::Connection, posts: &[Post]) -> Result<Vec<crate::stored::Attachment>, HostError> {
    crate::storage::db::ensure_attachments_schema(db).map_err(host_err)?;
    let ids: std::collections::HashSet<&str> = posts.iter().map(|post| post.post_id.as_str()).collect();
    let mut statement = db
        .prepare("SELECT post_id, url, local_path, mime_type, on_site FROM attachments ORDER BY id")
        .map_err(host_err)?;
    let rows = statement
        .query_map([], |row| {
            Ok(crate::stored::Attachment {
                post_id: row.get(0)?,
                url: row.get(1)?,
                kind: "file".into(),
                local_path: PathBuf::from(row.get::<_, String>(2)?),
                mime_type: row.get(3)?,
                on_site: row.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(host_err)?;
    let mut out = Vec::new();
    for row in rows {
        let row = row.map_err(host_err)?;
        if ids.contains(row.post_id.as_str()) {
            out.push(row);
        }
    }
    Ok(out)
}

fn write_export(
    path: &str,
    request: &ExportRequest,
    posts: &[Post],
    attachments: &[crate::stored::Attachment],
    profiles: &[crate::stored::Profile],
    hits: &[crate::stored::MentionHit],
) -> Result<(), HostError> {
    let format = match request.format {
        ExportFormat::Json => crate::export::Format::Json,
        ExportFormat::Html => crate::export::Format::Html,
    };
    if !request.compress {
        let doc = crate::export::json::build_json("", posts, attachments, profiles, hits, &|attachment| {
            attachment.local_path.display().to_string()
        });
        let rendered = crate::export::render(format, &doc).map_err(host_err)?;
        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(host_err)?;
        }
        return std::fs::write(path, rendered).map_err(host_err);
    }
    let dest = PathBuf::from(path);
    let parent = dest.parent().filter(|parent| !parent.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&parent).map_err(host_err)?;
    let stage = parent.join(format!("export-stage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    let att_dir = stage.join("attachments");
    std::fs::create_dir_all(&att_dir).map_err(host_err)?;
    for attachment in attachments {
        if let Some(name) = attachment.local_path.file_name()
            && attachment.local_path.is_file()
        {
            std::fs::copy(&attachment.local_path, att_dir.join(name)).map_err(host_err)?;
        }
    }
    let mut profiles = profiles.to_vec();
    if request.profile_images {
        for profile in &mut profiles {
            for slot in [&mut profile.avatar_path, &mut profile.banner_path] {
                let Some(src) = slot.clone().map(PathBuf::from) else { continue };
                let Some(name) = src.file_name() else { continue };
                let dir = att_dir.join(crate::config::PROFILE_IMAGES_DIR);
                if src.is_file() && std::fs::create_dir_all(&dir).is_ok() && std::fs::copy(&src, dir.join(name)).is_ok() {
                    *slot = Some(format!(
                        "attachments/{}/{}",
                        crate::config::PROFILE_IMAGES_DIR,
                        name.to_string_lossy()
                    ));
                }
            }
        }
    }
    let doc = crate::export::json::build_json("", posts, attachments, &profiles, hits, &|attachment| {
        format!(
            "attachments/{}",
            attachment.local_path.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    let rendered = crate::export::render(format, &doc).map_err(host_err)?;
    let name = format!("export.{}", format.ext());
    std::fs::write(stage.join(&name), rendered).map_err(host_err)?;
    let file = std::fs::File::create(&dest).map_err(host_err)?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    tar.append_dir_all("export", &stage).map_err(host_err)?;
    tar.into_inner().map_err(host_err)?.finish().map_err(host_err)?;
    let _ = std::fs::remove_dir_all(&stage);
    Ok(())
}

fn host_err(err: impl std::fmt::Display) -> HostError {
    HostError::new(err.to_string())
}

fn item_to_post(item: &ItemRec) -> Result<Post, HostError> {
    if item.source_id.native.is_empty() {
        return Err(HostError::new("an item needs a native id"));
    }
    Ok(Post {
        post_id: item.source_id.native.clone(),
        parent_post_id: item
            .parent
            .as_ref()
            .map(|id| id.native.clone())
            .unwrap_or_default(),
        in_reply_to_id: item
            .in_reply_to
            .as_ref()
            .map(|id| id.native.clone())
            .unwrap_or_default(),
        nick: item.nick.clone(),
        title: item.title.clone(),
        content: item.body.clone(),
        posted_at: item.posted_at.clone(),
        media: item
            .media
            .iter()
            .map(|media| crate::stored::MediaRef {
                url: media.url.clone(),
                kind: media.kind.clone(),
            })
            .collect(),
        truncated: item.body_state == BodyState::Truncated,
        body_json: item.body_json.clone(),
        score: item.score,
    })
}

fn post_to_item(post: &Post) -> ItemRec {
    let mut item = ItemRec::new(
        SourceId::new(acp_rs_plugin::ServiceId::new("redstar"), post.post_id.clone()),
        post.content.clone(),
    );
    item.nick = post.nick.clone();
    item.title = post.title.clone();
    item.body_json = post.body_json.clone();
    item.posted_at = post.posted_at.clone();
    item.truncated = post.truncated;
    item.score = post.score;
    if !post.parent_post_id.is_empty() {
        item.parent = Some(SourceId::new(
            acp_rs_plugin::ServiceId::new("redstar"),
            post.parent_post_id.clone(),
        ));
    }
    if !post.in_reply_to_id.is_empty() {
        item.in_reply_to = Some(SourceId::new(
            acp_rs_plugin::ServiceId::new("redstar"),
            post.in_reply_to_id.clone(),
        ));
    }
    item
}

fn account_to_profile(account: &AccountRec) -> Result<crate::stored::Profile, HostError> {
    if account.nick.is_empty() {
        return Err(HostError::new("an account needs a nick"));
    }
    let mut profile = crate::stored::Profile {
        nick: account.nick.clone(),
        user_id: Some(account.source_id.native.clone()),
        display_name: none_if_empty(&account.display_name),
        bio: none_if_empty(&account.bio),
        avatar_url: none_if_empty(&account.avatar_url),
        location: none_if_empty(&account.location),
        joined_at: account.joined_at.clone(),
        followers: account.followers,
        following: account.following,
        posts_count: account.posts_count,
        ..crate::stored::Profile::default()
    };
    if profile.scraped_at.is_empty() {
        profile.scraped_at = chrono::Local::now().to_rfc3339();
    }
    Ok(profile)
}

fn none_if_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{SegmentRec, ServiceId};

    fn temp_home(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "acp-host-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_page_round_trips_without_the_plugin_crate_opening_the_database() {
        let home = temp_home("page");
        let host = CoreHost::for_home(&home);
        let id = SourceId::new(ServiceId::new("redstar"), "post-1");
        let mut item = ItemRec::new(id.clone(), "hello from the service");
        item.nick = "ada".into();
        item.title = "Hello".into();
        item.posted_at = "2026-10-22T00:00:00Z".into();
        item.body_state = BodyState::Unknown;
        item.attachments.push(AttachmentRec {
            url: "https://example.test/a.png".into(),
            kind: "image".into(),
            mime: "image/png".into(),
            local_path: String::new(),
            on_site: true,
        });
        let mut reply = ItemRec::new(
            SourceId::new(ServiceId::new("redstar"), "reply-1"),
            "a reply",
        );
        reply.parent = Some(id.clone());
        reply.nick = "bob".into();
        let mut account = AccountRec::new(
            SourceId::new(ServiceId::new("redstar"), "user-1"),
            "ada",
        );
        account.display_name = "Ada".into();
        let segment = SegmentRec {
            source_id: SourceId::new(ServiceId::new("redstar"), "cue-1"),
            parent: id.clone(),
            body: "cue".into(),
            speaker: "ada".into(),
            start_ms: 0,
            end_ms: 10,
        };
        let page = Page {
            accounts: vec![account],
            items: vec![item, reply],
            segments: vec![segment],
            next: None,
            cost: acp_rs_plugin::Spend::default(),
        };
        assert_eq!(host.save_page(&page).unwrap(), 3);
        let loaded = host.load_item(&id).unwrap().unwrap();
        assert_eq!(loaded.body, "hello from the service");
        assert_eq!(loaded.nick, "ada");
        assert_eq!(loaded.source_id.native, "post-1");
        assert_eq!(loaded.body_state, BodyState::Unknown);
        assert_eq!(loaded.attachments.len(), 1);
        assert_eq!(loaded.attachments[0].url, "https://example.test/a.png");
        assert_eq!(loaded.attachments[0].mime, "image/png");
        let known = host.known_ids().unwrap();
        assert!(known.ids.iter().any(|id| id == "post-1"));
        assert_eq!(
            known
                .reply_counts
                .iter()
                .find(|row| row.thread_id == "post-1")
                .map(|row| row.replies),
            Some(2)
        );

        host.store_secret("session", b"sealed").unwrap();
        assert_eq!(host.load_secret("session").unwrap().unwrap(), b"sealed");

        host.add_bookmark("Abcd1234").unwrap();
        assert_eq!(host.list_bookmarks().unwrap(), vec!["Abcd1234".to_string()]);
        assert_eq!(host.list_outbox().unwrap(), vec!["outbox none".to_string()]);

        let notice_id = host
            .push_notice(&NoticeRec {
                kind: "sync".into(),
                title: "saved".into(),
                body: "one page".into(),
                source_id: Some(id.clone()),
            })
            .unwrap();
        assert!(notice_id > 0);

        host.set_pref("email", "ada@example.test").unwrap();
        assert_eq!(host.email().unwrap(), "ada@example.test");
        assert!(host.base_url().unwrap().starts_with("http"));
        assert!(host.set_pref("base_url", "https://example.test").is_err());
        assert_eq!(host.argv0(), "acp-red-star");
        assert!(host.database_path().unwrap().ends_with("rs.database"));

        host.add_research("notes", "post-1", "keep").unwrap();
        let preview = host.preview_rules().unwrap();
        assert!(preview.is_empty() || preview.contains(' '));
        let summary = host.remove_nick("nobody", true, false).unwrap();
        assert!(summary.contains("would remove"), "{summary}");

        let export = home.join("page.json");
        host.export(&export.display().to_string(), &ExportRequest::new(ExportFormat::Json))
            .unwrap();
        let html = home.join("page.html");
        host.export(&html.display().to_string(), &ExportRequest::new(ExportFormat::Html))
            .unwrap();
        let html_text = std::fs::read_to_string(&html).unwrap();
        assert!(html_text.contains("hello from the service"), "{html_text}");
        assert!(html_text.contains('<'));
        let other = CoreHost::for_home(temp_home("import"));
        let n = other.import_json(&export.display().to_string()).unwrap();
        assert!(n >= 1, "{n}");
        let again = other.load_item(&id).unwrap().unwrap();
        assert_eq!(again.body, "hello from the service");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn http_uses_the_core_client_against_a_local_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = std::io::Read::read(&mut sock, &mut buf).unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]).into_owned();
            let body = text.into_bytes();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut sock, head.as_bytes());
            let _ = std::io::Write::write_all(&mut sock, &body);
        });
        let host = CoreHost::for_home(temp_home("http"));
        let url = format!("http://127.0.0.1:{port}/");
        assert!(host.http_get("redstar", "", &[]).is_err());
        assert!(host.http_get("redstar", "file:///etc/passwd", &[]).is_err());
        host.set_cookie("redstar", &url, "sid", "abc").unwrap();
        assert!(host.cookies("other").unwrap().is_empty());
        let stored = host.cookies("redstar").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].value, "abc");
        let response = host
            .http_get(
                "redstar",
                &url,
                &[HttpHeader::new("X-Test", "yes")],
            )
            .unwrap();
        assert_eq!(response.status, 200);
        let echoed = String::from_utf8_lossy(&response.body);
        assert!(echoed.to_ascii_lowercase().contains("cookie:"), "{echoed}");
        assert!(echoed.contains("sid=abc"), "{echoed}");
        assert!(echoed.to_ascii_lowercase().contains("x-test: yes"), "{echoed}");
        host.clear_cookies("redstar").unwrap();
        assert!(host.cookies("redstar").unwrap().is_empty());
    }

    #[test]
    fn login_cookie_survives_set_cookie_and_the_next_request() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut sock, _)) = listener.accept() else { break };
                let text = read_local_request(&mut sock);
                let (status, extra, body) = if text.starts_with("POST /login") {
                    (
                        "HTTP/1.1 302 Found",
                        format!(
                            "Location: http://127.0.0.1:{port}/home\r\nSet-Cookie: session=s3cret; Path=/; HttpOnly\r\n"
                        ),
                        String::new(),
                    )
                } else if text.starts_with("GET /home") {
                    ("HTTP/1.1 200 OK", "Content-Type: text/plain\r\n".into(), "home".into())
                } else if text.starts_with("GET /again") {
                    ("HTTP/1.1 200 OK", "Content-Type: text/plain\r\n".into(), text)
                } else {
                    ("HTTP/1.1 404 Not Found", "Content-Type: text/plain\r\n".into(), "no".into())
                };
                let head = format!(
                    "{status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut sock, head.as_bytes());
                let _ = std::io::Write::write_all(&mut sock, body.as_bytes());
            }
        });
        let host = CoreHost::for_home(temp_home("login-jar"));
        let login = format!("http://127.0.0.1:{port}/login");
        let response = host
            .http_post(
                "redstar",
                &login,
                &[],
                b"email=ada",
                "application/x-www-form-urlencoded",
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert!(response.url.ends_with("/home"), "{}", response.url);
        assert!(
            response.headers.iter().any(|header| {
                header.name.eq_ignore_ascii_case("location") && header.value.contains("/home")
            }),
            "{:?}",
            response.headers
        );
        assert!(
            response.set_cookie_names.iter().any(|name| name == "session"),
            "{:?}",
            response.set_cookie_names
        );
        assert!(!format!("{response:?}").contains("s3cret"), "{response:?}");
        let stored = host.cookies("redstar").unwrap();
        assert!(
            stored.iter().any(|cookie| cookie.name == "session" && cookie.value == "s3cret"),
            "{stored:?}"
        );
        assert!(!format!("{stored:?}").contains("s3cret"), "{stored:?}");
        host.set_cookie("redstar", &format!("http://127.0.0.1:{port}/"), "theme", "dark")
            .unwrap();
        let stored = host.cookies("redstar").unwrap();
        assert!(stored.iter().any(|cookie| cookie.name == "session" && cookie.value == "s3cret"));
        assert!(stored.iter().any(|cookie| cookie.name == "theme" && cookie.value == "dark"));
        let again = host
            .http_get("redstar", &format!("http://127.0.0.1:{port}/again"), &[])
            .unwrap();
        let echoed = String::from_utf8_lossy(&again.body);
        assert!(echoed.contains("session=s3cret"), "{echoed}");
        assert!(echoed.contains("theme=dark"), "{echoed}");
    }

    fn read_local_request(sock: &mut std::net::TcpStream) -> String {
        sock.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            match std::io::Read::read(sock, &mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(header_end) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header_text = String::from_utf8_lossy(&buf[..header_end]);
                        let length = header_text
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                if name.eq_ignore_ascii_case("content-length") {
                                    value.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if buf.len() - (header_end + 4) >= length {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[test]
    fn outbox_bookmarks_and_a_chosen_export_stay_on_the_host() {
        let home = temp_home("rich");
        let host = CoreHost::for_home(&home);
        let saved = host
            .save_outbox("draft-1", "Hello", "body text", "", false)
            .unwrap();
        assert!(saved.contains("state=draft"), "{saved}");
        let queued = host.enqueue_outbox("draft-1").unwrap();
        assert!(queued.contains("status=queued"), "{queued}");
        let lines = host.list_outbox().unwrap();
        assert!(lines.iter().any(|line| line.contains("status=queued")), "{lines:?}");
        let cancelled = host.cancel_outbox("Q-draft-1").unwrap();
        assert!(cancelled.contains("status=cancelled"), "{cancelled}");

        host.add_bookmark("local-1").unwrap();
        let sync = host
            .sync_bookmarks("redstar", &[SiteBookmark::new("site-9", "post-9")], true)
            .unwrap();
        assert_eq!(sync.pulled, 1);
        assert_eq!(sync.push_add.len(), 1);
        assert_eq!(sync.push_add[0].post_id, "local-1");
        host.bookmark_pushed("redstar", "local-1", "site-1", true).unwrap();
        assert!(host.list_bookmarks().unwrap().iter().any(|id| id == "local-1"));
        assert!(host.list_bookmarks().unwrap().iter().any(|id| id == "post-9"));

        let id = SourceId::new(ServiceId::new("redstar"), "post-1");
        let mut item = ItemRec::new(id, "hello from the service");
        item.nick = "ada".into();
        item.title = "Hello".into();
        let other = ItemRec::new(SourceId::new(ServiceId::new("redstar"), "post-2"), "second");
        let account = AccountRec::new(SourceId::new(ServiceId::new("redstar"), "user-ada"), "ada");
        let page = Page {
            items: vec![item, other],
            accounts: vec![account],
            ..Page::empty()
        };
        host.save_page(&page).unwrap();
        let image = home.join("ada.png");
        std::fs::write(&image, b"png").unwrap();
        let db = crate::storage::db::open_db(&home.join(crate::config::DB_REL_PATH)).unwrap();
        db.execute(
            "UPDATE profiles SET avatar_path = ?1 WHERE nick = 'ada'",
            rusqlite::params![image.display().to_string()],
        )
        .unwrap();
        let mut request = ExportRequest::new(ExportFormat::Json);
        request.post_ids = vec!["post-1".into()];
        request.profile_images = true;
        request.compress = true;
        let archive = home.join("chosen.tar.gz");
        host.export(&archive.display().to_string(), &request).unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let mut archive = tar::Archive::new(decoder);
        let mut saw_post = false;
        let mut saw_profile = false;
        let mut saw_image = false;
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().to_string();
            let mut text = String::new();
            std::io::Read::read_to_string(&mut entry, &mut text).unwrap();
            if name.ends_with("export.json") {
                assert!(text.contains("hello from the service"), "{text}");
                assert!(!text.contains("second"), "{text}");
                assert!(text.contains("ada"), "{text}");
                saw_post = true;
                saw_profile = text.contains("avatar_path");
            }
            if name.contains("ada.png") {
                saw_image = true;
            }
        }
        assert!(saw_post && saw_profile && saw_image, "post={saw_post} profile={saw_profile} image={saw_image}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
