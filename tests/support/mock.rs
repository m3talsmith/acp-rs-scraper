//! A mock acp.us: just enough HTTP + Phoenix LiveView websocket behaviour for
//! the scraper to log in, page feeds, read threads, switch the profile filter
//! and download media. It never talks to the real site.
//!
//! Fixture (newest first within each feed):
//! - alice: root posts A1..A5 (A1 has an image), and reply AR1 in bob's thread B1
//! - bob: root B1 ("hello @alice ..."), reply R1 in alice's thread A1
//! - carol: root C1 (unrelated)
//! - dave: replies DR1/DR2 in C1 (HTML body / no feed body)
//! - erin: root E1 and reply ER1, both truncated in feeds (full on the thread page)

use axum::{
    Router,
    extract::{Path, Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::{Arc, Mutex}};

/// One-time code the mock accepts.
pub const CODE: &str = "KWMRUV";
/// Session cookie name set after sign-in.
const SESSION: &str = "_mock_session";
/// Bumped each time the fixture is built, so media URLs carry a new signature.
static SIGNED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Items per feed page.
const PAGE: usize = 2;

/// Counters and switches the steps read or flip.
#[derive(Default, Debug)]
pub struct Inner {
    /// Cookie headers sent with `/web/...` requests (research web pages must not carry the session).
    pub web_cookies: Vec<String>,
    /// Feed `page-next` events received.
    pub page_next: usize,
    /// Thread pages served.
    pub thread_gets: usize,
    /// `configure-feed` events received.
    pub configure_feed: usize,
    /// Requests for `/rs/@nick?replies=1`.
    pub replies_url_gets: usize,
    /// When set, `/rs/@nick?replies=1` answers 500 (forces the form fallback).
    pub replies_url_broken: bool,
    /// Emails a login code was "sent" to.
    pub login_emails: Vec<String>,
    /// Posts bookmarked on the site (starts with `C1`).
    pub site_bookmarks: Vec<String>,
    /// `bookmark` hook events received.
    pub bookmark_toggles: usize,
    /// How `/themes/default.toml` answers: "ok" (the repo file), "big", "bad", "slow", "missing".
    pub theme_mode: String,
    /// `/themes/...` requests received.
    pub theme_gets: usize,
    /// Cookie headers sent with `/themes/...` requests (there must be none).
    pub theme_cookies: Vec<String>,
    /// Reply-edit `validate` events. A top-level post edit must stay at 0.
    pub edit_validates: usize,
    /// Reply-edit `save` events. One confirmed reply edit sends one.
    pub edit_saves: usize,
    /// Form value of the last reply-edit validate.
    pub edit_validate_value: String,
    /// Form value of the last reply-edit save.
    pub edit_save_value: String,
    /// Account writes (profile, settings, report). A gated client leaves this at 0.
    pub account_writes: usize,
}

/// Shared mock state.
#[derive(Clone, Default)]
pub struct Mock {
    /// Base URL, e.g. `http://127.0.0.1:12345`.
    pub base: String,
    /// Counters and switches.
    pub inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for Mock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Mock({})", self.base) }
}

fn body(text: &str) -> Value {
    json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]})
}

fn paras(ps: &[&str]) -> Value {
    json!({"type": "doc", "content": ps.iter().map(|t| json!({"type": "paragraph", "content": [{"type": "text", "text": t}]})).collect::<Vec<_>>()})
}

/// Every post on the mock site.
fn posts(base: &str) -> Vec<Value> {
    let author = |n: &str| json!({"username": n, "user_id": format!("u-{n}"), "display_name": format!("{} Example", n.to_uppercase()),
        "avatar_url": format!("{base}/media/avatar-{n}.png")});
    let root = |id: &str, n: &str, day: u32, text: &str, replies: u32| json!({
        "id": id, "thread_id": id, "author": author(n), "title": format!("{id} title"), "body": body(text),
        "inserted_at": format!("2026-10-{day:02}T12:00:00Z"), "replies_count": replies, "media": []});
    let reply = |id: &str, thread: &str, n: &str, day: u32, text: &str| json!({
        "id": id, "thread_id": thread, "author": author(n), "title": "", "body": body(text),
        "inserted_at": format!("2026-10-{day:02}T13:00:00Z")});
    let mut a1 = root("A1", "alice", 1, "first post from alice", 1);
    // Like the live site, media links are presigned and re-signed on every load.
    let sig = SIGNED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    a1["media"] = json!([{"type": "image", "url": format!("{base}/media/a1.png?X-Amz-Date={sig}&X-Amz-Signature=s{sig}")}]);
    vec![
        a1,
        root("A2", "alice", 2, "second", 0),
        root("A3", "alice", 3, "third", 0),
        root("A4", "alice", 4, "fourth", 0),
        root("A5", "alice", 5, "fifth general strike", 0),
        root("B1", "bob", 6, "hello @alice how are you", 1),
        root("C1", "carol", 7, "unrelated", 0),
        reply("R1", "A1", "bob", 8, "bob replies to alice"),
        reply("AR1", "B1", "alice", 9, "alice replies to bob"),
        // dave only replies, in shapes seen on the live site: an HTML-string
        // body, and a feed item with no body at all (only the thread has it).
        { let mut r = reply("DR1", "C1", "dave", 10, ""); r["body"] = json!("<p>dave &amp; the <b>html</b> body</p>"); r }
        ,
        // DR2 answers DR1 (a comment, not the root): the site marks that with a
        // `postRef` node in the body; the thread itself stays flat (thread_id C1).
        { let mut r = reply("DR2", "C1", "dave", 11, "dave full reply from the thread"); r["feed_body_missing"] = json!(true);
          r["body"]["content"].as_array_mut().unwrap().insert(0, json!({"type": "postRef", "attrs": {"postId": "DR1", "threadId": "C1"}})); r },
        // erin writes long posts: feeds send them shortened (`body_truncated`,
        // first paragraph only plus `body_text_summary`); the thread page has it all.
        { let mut r = root("E1", "erin", 12, "", 1); r["body"] = paras(&["Erin para one.", "Erin para two.", "Erin para three ends here."]);
          r["feed_truncated"] = json!(true); r },
        { let mut r = reply("ER1", "E1", "erin", 13, ""); r["body"] = paras(&["Erin reply one.", "Erin reply two ends here."]);
          r["feed_truncated"] = json!(true); r },
    ]
}

/// Posts of a feed, newest first. `who` = Some(nick) for a profile.
fn feed(base: &str, who: Option<&str>, replies: bool) -> Vec<Value> {
    let mut v: Vec<Value> = posts(base).into_iter().filter(|p| {
        let root = p["id"] == p["thread_id"];
        match who {
            Some(n) => p["author"]["username"] == n && (root || replies),
            None => root,
        }
    }).collect();
    for p in &mut v {
        if p["feed_body_missing"] == true { p["body"] = Value::Null; }
        if p["feed_truncated"] == true {
            let first = p["body"]["content"][0].clone();
            let summary = first["content"][0]["text"].as_str().unwrap_or("").to_string() + "…";
            p["body"] = json!({"type": "doc", "content": [first]});
            p["body_truncated"] = json!(true);
            p["truncate"] = json!(true);
            p["body_text_summary"] = json!(summary);
        }
        if let Some(m) = p.as_object_mut() { m.remove("feed_truncated"); m.remove("feed_body_missing"); }
    }
    v.sort_by(|a, b| b["inserted_at"].as_str().cmp(&a["inserted_at"].as_str()));
    v
}

fn page_of(items: &[Value], from: usize) -> Value {
    let end = (from + PAGE).min(items.len());
    let at_end = end >= items.len();
    json!({"posts": &items[from.min(items.len())..end], "next_cursor": if at_end { Value::Null } else { json!(end.to_string()) }, "at_end": at_end})
}

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;") }

fn lv_page(inner: &str) -> String {
    format!(r#"<!DOCTYPE html><html><head><meta name="csrf-token" content="mock-csrf"></head><body>
<div data-phx-main id="phx-mock" data-phx-session="mock-session" data-phx-static="mock-static">{inner}</div></body></html>"#)
}

fn logged_in(h: &HeaderMap) -> bool {
    h.get_all(header::COOKIE).iter().any(|c| c.to_str().unwrap_or("").contains(&format!("{SESSION}=ok")))
}

fn to_auth(path: &str) -> Response {
    (StatusCode::FOUND, [(header::LOCATION, format!("/auth?return_to={}", path.replace('/', "%2F").replace('@', "%40")))]).into_response()
}

fn html(s: String) -> Response { ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], s).into_response() }

async fn site_feed(State(m): State<Mock>, h: HeaderMap) -> Response {
    if !logged_in(&h) { return to_auth("/rs"); }
    let props = page_of(&feed(&m.base, None, false), 0);
    html(lv_page(&format!(r#"<div id="forum/Feed" data-props="{}"></div>"#, esc(&props.to_string()))))
}

fn bookmark_feed(m: &Mock) -> Vec<Value> {
    let marks = m.inner.lock().unwrap().site_bookmarks.clone();
    let mut v: Vec<Value> = posts(&m.base).into_iter().filter(|p| marks.iter().any(|b| p["id"] == b.as_str())).collect();
    for p in &mut v { if let Some(o) = p.as_object_mut() { o.remove("feed_truncated"); o.remove("feed_body_missing"); } }
    v
}

async fn bookmarks_page(State(m): State<Mock>, h: HeaderMap) -> Response {
    if !logged_in(&h) { return to_auth("/rs/bookmarks"); }
    let props = page_of(&bookmark_feed(&m), 0);
    html(lv_page(&format!(r#"<div id="forum/Feed" data-props="{}"></div>"#, esc(&props.to_string()))))
}

async fn profile(State(m): State<Mock>, h: HeaderMap, Path(at): Path<String>, Query(q): Query<HashMap<String, String>>) -> Response {
    let Some(nick) = at.strip_prefix('@') else { return StatusCode::NOT_FOUND.into_response() };
    if !logged_in(&h) { return to_auth(&format!("/rs/{at}")); }
    let replies = q.get("replies").map(String::as_str) == Some("1");
    if replies {
        let mut i = m.inner.lock().unwrap();
        i.replies_url_gets += 1;
        if i.replies_url_broken { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); }
    }
    let props = page_of(&feed(&m.base, Some(nick), replies), 0);
    html(lv_page(&format!(
        r#"<div id="js-profileCard" data-username="{0}" data-display-name="{1} Example" data-bio="organizing in the shop"></div><h1 class="ProfileName">{1} Example</h1><dialog id="js-configureFeed"><form phx-submit="configure-feed">
<input type="radio" name="replies" value="0" checked> Posts <input type="radio" name="replies" value="1"> Posts and replies</form></dialog>
<div id="forum/Feed" data-props="{2}"></div>"#, nick, nick.to_uppercase(), esc(&props.to_string()))))
}

async fn settings_page(h: HeaderMap) -> Response {
    if !logged_in(&h) { return to_auth("/rs/settings"); }
    html(lv_page(r#"<form id="settings-form" phx-submit="save" method="post">
<input name="form[time_zone_id]" value="America/Chicago">
<input name="form[theme]" value="dark">
<input name="form[show_sustainer?]" value="true">
</form>"#))
}

async fn thread(State(m): State<Mock>, h: HeaderMap, Path(id): Path<String>) -> Response {
    if !logged_in(&h) { return to_auth(&format!("/rs/post/{id}")); }
    m.inner.lock().unwrap().thread_gets += 1;
    let all = posts(&m.base);
    let Some(root) = all.iter().find(|p| p["id"] == id.as_str()).cloned() else { return StatusCode::NOT_FOUND.into_response() };
    let replies: Vec<Value> = all.iter().filter(|p| p["thread_id"] == id.as_str() && p["id"] != id.as_str()).cloned().collect();
    let mut root = root;
    let mut replies = replies;
    for p in std::iter::once(&mut root).chain(replies.iter_mut()) {
        if let Some(m) = p.as_object_mut() { m.remove("feed_truncated"); m.remove("feed_body_missing"); m.insert("body_truncated".into(), json!(false)); }
    }
    let props = json!({"root": root, "replies": replies});
    html(lv_page(&format!(r#"<div id="forum/Thread" data-props="{}"></div>"#, esc(&props.to_string()))))
}

async fn auth_page() -> Response { html(lv_page(r#"<div id="auth-page"></div>"#)) }

async fn sign_in(body: String) -> Response {
    let ok = body.split('&').any(|kv| kv == format!("user%5Bcode%5D={CODE}") || kv == format!("user[code]={CODE}"));
    if !ok { return (StatusCode::FORBIDDEN, "bad code").into_response(); }
    (StatusCode::FOUND, [(header::SET_COOKIE, format!("{SESSION}=ok; Path=/; HttpOnly")), (header::LOCATION, "/rs".into())]).into_response()
}

async fn media(Path(name): Path<String>) -> Response {
    // 1x1 PNG
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89];
    let _ = name;
    ([(header::CONTENT_TYPE, "image/png")], PNG).into_response()
}

/// The repo's `themes/` as raw.githubusercontent.com would serve it (`theme_mode`
/// changes how `default.toml` answers).
async fn theme_file(State(m): State<Mock>, headers: HeaderMap, Path(file): Path<String>) -> Response {
    let mode = {
        let mut i = m.inner.lock().unwrap();
        i.theme_gets += 1;
        if let Some(c) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) { i.theme_cookies.push(c.to_string()); }
        i.theme_mode.clone()
    };
    let plain = |s: String| ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], s).into_response();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("themes").join(&file);
    if file == "default.toml" {
        match mode.as_str() {
            "big" => return plain(format!("version = 1\n#{}\n", "x".repeat(70 * 1024))),
            "bad" => return plain("version = [oops".into()),
            "missing" => return (StatusCode::NOT_FOUND, "404: Not Found").into_response(),
            "slow" => tokio::time::sleep(std::time::Duration::from_secs(5)).await,
            _ => {}
        }
    }
    match std::fs::read_to_string(&repo) {
        Ok(s) if !file.contains('/') => plain(s),
        _ => (StatusCode::NOT_FOUND, "404: Not Found").into_response(),
    }
}

/// A third-party article for research `--url` tests.
async fn web_article(State(m): State<Mock>, headers: axum::http::HeaderMap) -> Response {
    if let Some(c) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) { m.inner.lock().unwrap().web_cookies.push(c.to_string()); }
    let html = r#"<!DOCTYPE html><html><head><title>The Great Railroad Strike</title></head><body>
<nav><a href="/">Home</a> menu text</nav>
<article><h1>The Great Railroad Strike of 1877</h1><p>Workers stopped the trains in <b>Martinsburg</b>.</p>
<ul><li>Baltimore</li><li>Pittsburgh</li></ul><blockquote>An injury to one is an injury to all.</blockquote></article>
<footer>footer text</footer><script>var tracking = 1;</script></body></html>"#;
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

async fn live(State(m): State<Mock>, ws: WebSocketUpgrade) -> Response { ws.on_upgrade(move |s| lv_conn(m, s)) }

/// Which feed a joined LiveView pages: from its join URL and any `configure-feed`.
struct Conn {
    nick: Option<String>,
    replies: bool,
    auth: bool,
    bookmarks: bool,
    /// Reply id when this socket joined an edit page.
    edit_id: Option<String>,
    /// Thread of that reply. Empty when the id is a root post.
    edit_thread: Option<String>,
}

const REPLY_EDIT_FORM: &str = r#"<div data-composer="true" data-mode="reply"><form id="js-postForm-form" method="put" phx-submit="save" phx-change="validate" phx-target="1"><textarea name="form[body_input]"></textarea></form></div>"#;
const POST_EDIT_FORM: &str = r#"<div data-composer="true" data-mode="post"><form id="js-postForm-form" method="put" phx-submit="save" phx-change="validate" phx-target="1"><input name="form[title]" value=""><textarea name="form[body_input]"></textarea></form></div>"#;

fn edit_id_from(redirect: &str) -> Option<String> {
    let path = redirect.split('?').next().unwrap_or(redirect).trim_end_matches('/');
    let mut parts = path.rsplit('/');
    let last = parts.next()?;
    if last != "edit" {
        return None;
    }
    let id = parts.next().filter(|s| !s.is_empty())?;
    Some(id.to_string())
}

fn reply_thread(id: &str) -> Option<&'static str> {
    match id {
        "R1" => Some("A1"),
        "AR1" => Some("B1"),
        "DR1" | "DR2" => Some("C1"),
        "ER1" => Some("E1"),
        _ => None,
    }
}

fn edit_form_value(payload: &Value) -> Option<&str> {
    let value = payload["value"].as_str()?;
    let body = value.contains("form[body_input]=") || value.contains("form%5Bbody_input%5D=");
    let other = value.contains("form[title]") || value.contains("form%5Btitle%5D") || value.contains("form[thread_id]") || value.contains("form%5Bthread_id%5D");
    (body && !other).then_some(value)
}

fn reply(msg: &Value, response: Value) -> Message {
    Message::Text(json!([msg[0], msg[1], msg[2], "phx_reply", {"status": "ok", "response": response}]).to_string().into())
}

async fn lv_conn(m: Mock, mut ws: WebSocket) {
    let mut c = Conn { nick: None, replies: false, auth: false, bookmarks: false, edit_id: None, edit_thread: None };
    while let Some(Ok(Message::Text(t))) = ws.next().await {
        let Ok(msg) = serde_json::from_str::<Value>(&t) else { continue };
        let (event, payload) = (msg[3].as_str().unwrap_or(""), &msg[4]);
        let outs = match event {
            "phx_join" => {
                let redirect = payload["redirect"].as_str().unwrap_or("");
                if let Some(id) = edit_id_from(redirect) {
                    c.edit_thread = reply_thread(&id).map(str::to_string);
                    c.edit_id = Some(id);
                    let form = if c.edit_thread.is_some() { REPLY_EDIT_FORM } else { POST_EDIT_FORM };
                    vec![reply(&msg, json!({"rendered": {"s": [form]}}))]
                } else {
                    let url = payload["url"].as_str().unwrap_or("");
                    c.auth = url.contains("/auth");
                    c.replies = url.contains("replies=1");
                    c.bookmarks = url.contains("/rs/bookmarks");
                    c.nick = url.split("/rs/@").nth(1).map(|r| r.split(['?', '/']).next().unwrap_or("").to_string());
                    let s = if c.auth { r#"<form phx-submit="send-login-email" phx-target="1"><input type="email" name="user[email]"></form>"# } else { "<div></div>" };
                    vec![reply(&msg, json!({"rendered": {"s": [s]}}))]
                }
            }
            "event" => {
                let blob = payload.to_string();
                if blob.contains("edit-profile-form") || blob.contains("settings-form") || blob.contains("js-reportPostActions") || blob.contains("js-reportUserActions") || blob.contains("form[display_name]") || blob.contains("form[forum_profile]") || blob.contains("form[time_zone_id]") || blob.contains("form[show_sustainer") {
                    m.inner.lock().unwrap().account_writes += 1;
                }
                match (payload["type"].as_str(), payload["event"].as_str()) {
                (Some("form"), Some("send-login-email")) => {
                    let v = payload["value"].as_str().unwrap_or("");
                    m.inner.lock().unwrap().login_emails.push(v.trim_start_matches("user%5Bemail%5D=").replace("%40", "@"));
                    vec![reply(&msg, json!({"diff": {"s": [r#"<form phx-submit="verify-code"><input type="text" name="user[code]"></form>"#]}}))]
                }
                (Some("form"), Some("verify-code")) => vec![reply(&msg, json!({"diff": {"s": [
                    r#"<form phx-trigger-action action="/auth/user/otp/sign_in" method="post"><input type="hidden" name="user[email]" value=""><input type="hidden" name="user[code]" value=""></form>"#]}}))],
                (Some("form"), Some("validate")) if edit_form_value(payload).is_some() => {
                    let value = edit_form_value(payload).unwrap_or("").to_string();
                    let mut i = m.inner.lock().unwrap();
                    i.edit_validates += 1;
                    i.edit_validate_value = value;
                    vec![reply(&msg, json!({"diff": {}}))]
                }
                (Some("form"), Some("save")) if edit_form_value(payload).is_some() => {
                    let value = edit_form_value(payload).unwrap_or("").to_string();
                    {
                        let mut i = m.inner.lock().unwrap();
                        i.edit_saves += 1;
                        i.edit_save_value = value;
                    }
                    let id = c.edit_id.clone().unwrap_or_default();
                    let thread = c.edit_thread.clone().unwrap_or_else(|| id.clone());
                    let to = format!("/rs/post/{thread}/{id}");
                    vec![
                        reply(&msg, json!({"diff": {}})),
                        Message::Text(json!([msg[0], serde_json::Value::Null, msg[2], "live_redirect", {"kind": "replace", "to": to}]).to_string().into()),
                    ]
                }
                (Some("form"), Some("configure-feed")) => {
                    m.inner.lock().unwrap().configure_feed += 1;
                    c.replies = payload["value"].as_str().unwrap_or("").contains("replies=1");
                    let items = feed(&m.base, c.nick.as_deref(), c.replies);
                    vec![reply(&msg, json!({"diff": {"r": page_of(&items, 0)}}))]
                }
                (Some("hook"), Some("page-next")) => {
                    m.inner.lock().unwrap().page_next += 1;
                    let from = payload["value"]["cursor"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let items = if c.bookmarks { bookmark_feed(&m) } else { feed(&m.base, c.nick.as_deref(), c.replies) };
                    vec![reply(&msg, json!({"diff": {"r": page_of(&items, from)}}))]
                }
                (Some("hook"), Some("bookmark")) => {
                    let id = payload["value"]["id"].as_str().unwrap_or("").to_string();
                    let state = payload["value"]["state"].as_str().unwrap_or("");
                    let mut i = m.inner.lock().unwrap();
                    i.bookmark_toggles += 1;
                    match state {
                        "1" => { if !i.site_bookmarks.iter().any(|b| b == &id) { i.site_bookmarks.push(id); } }
                        "0" => i.site_bookmarks.retain(|b| b != &id),
                        _ => {}
                    }
                    vec![reply(&msg, json!({"diff": {}}))]
                }
                _ => vec![reply(&msg, json!({"diff": {}}))],
                }
            },
            _ => vec![reply(&msg, json!({}))],
        };
        for out in outs {
            if ws.send(out).await.is_err() { return; }
        }
    }
}

impl Mock {
    /// Start the mock on an ephemeral localhost port.
    pub async fn start() -> Mock {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut m = Mock { base: format!("http://{}", listener.local_addr().unwrap()), ..Default::default() };
        m.inner = Arc::new(Mutex::new(Inner { site_bookmarks: vec!["C1".into()], ..Default::default() }));
        let app = Router::new()
            .route("/rs", get(site_feed))
            .route("/rs/latest", get(site_feed))
            .route("/rs/bookmarks", get(bookmarks_page))
            .route("/rs/settings", get(settings_page))
            .route("/rs/{at}", get(profile))
            .route("/rs/post/{id}", get(thread))
            .route("/auth", get(auth_page))
            .route("/auth/user/otp/sign_in", post(sign_in))
            .route("/media/{name}", get(media))
            .route("/live/websocket", get(live))
            .route("/web/article", get(web_article))
            .route("/themes/{file}", get(theme_file))
            .with_state(m.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        m
    }
}
