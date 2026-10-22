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
    AccountRec, AttachmentRec, BodyState, CookieRec, ExportFormat, Host, HostError, HttpHeader,
    HttpResponse, ItemRec, KnownIds, NoticeRec, Page, SourceId, ThreadReplies,
};
use rusqlite::{Connection, params};

use crate::config::{ATTACH_REL_PATH, DB_REL_PATH, PREFS_REL_PATH};
use crate::prefs::{Prefs, UiPrefs};
use crate::stored::Post;

const SECRET_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS host_secrets (
    field TEXT PRIMARY KEY,
    material BLOB NOT NULL
);";

/// Binary name. [`CoreHost::argv0`] returns this and does not spawn a process.
pub const ARGV0: &str = "acp-red-star";

struct ServiceHttp {
    jar: Arc<reqwest::cookie::Jar>,
    cookies: Vec<CookieRec>,
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

    fn export(&self, path: &str, format: ExportFormat) -> Result<(), HostError> {
        let db = self.open()?;
        crate::storage::db::ensure_posts_schema(&db).map_err(host_err)?;
        let posts = crate::rules::load_posts(&db).map_err(host_err)?;
        let doc = crate::export::json::build_json("", &posts, &[], &[], &[], &|_| String::new());
        let rendered = match format {
            ExportFormat::Json => crate::export::render(crate::export::Format::Json, &doc),
            ExportFormat::Html => crate::export::render(crate::export::Format::Html, &doc),
        }
        .map_err(host_err)?;
        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(host_err)?;
        }
        std::fs::write(path, rendered).map_err(host_err)
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
        if let Some(existing) = slot.cookies.iter_mut().find(|cookie| cookie.name == name && cookie.url == url) {
            existing.value = value.to_string();
        } else {
            slot.cookies.push(CookieRec {
                name: name.to_string(),
                value: value.to_string(),
                url: url.to_string(),
            });
        }
        slot.rebuild().map_err(host_err)?;
        Ok(())
    }

    fn cookies(&self, service: &str) -> Result<Vec<CookieRec>, HostError> {
        check_service(service)?;
        let http = self.http.lock().unwrap_or_else(|err| err.into_inner());
        Ok(http.get(service).map(|slot| slot.cookies.clone()).unwrap_or_default())
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
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(host_err)?;
            runtime.block_on(async move {
                let client = reqwest::Client::builder()
                    .cookie_provider(jar)
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
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = response.bytes().await.map_err(host_err)?.to_vec();
                Ok(HttpResponse {
                    status,
                    body,
                    content_type,
                })
            })
        });
        match worker.join() {
            Ok(result) => result,
            Err(_) => Err(HostError::new("the HTTP worker stopped")),
        }
    }
}

impl ServiceHttp {
    fn empty() -> Self {
        Self {
            jar: Arc::new(reqwest::cookie::Jar::default()),
            cookies: Vec::new(),
        }
    }

    fn rebuild(&mut self) -> Result<(), String> {
        let jar = reqwest::cookie::Jar::default();
        for cookie in &self.cookies {
            let url = reqwest::Url::parse(&cookie.url).map_err(|err| err.to_string())?;
            jar.add_cookie_str(&format!("{}={}", cookie.name, cookie.value), &url);
        }
        self.jar = Arc::new(jar);
        Ok(())
    }
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
        host.export(&export.display().to_string(), ExportFormat::Json)
            .unwrap();
        let html = home.join("page.html");
        host.export(&html.display().to_string(), ExportFormat::Html)
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
}
