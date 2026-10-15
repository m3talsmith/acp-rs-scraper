//! ACP Red Star (<https://acp.us/rs>) scraper.
//!
//! The login page is a Phoenix LiveView: the email form is submitted over the
//! LiveView websocket (`phx-submit="send-login-email"`), not via HTTP POST.

use anyhow::{Context, Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use reqwest::cookie::{CookieStore, Jar};
use rusqlite::{Connection, params};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use url::Url;

// ---------------------------------------------------------------- config
const BASE: &str = "https://acp.us";
const START_URL: &str = "https://acp.us/rs";
/// Single place to change the DB location (spec also mentioned ~/.acp/ai/rs.database).
const DB_REL_PATH: &str = ".acp/ai/data/rs.database";
const ATTACH_REL_PATH: &str = ".acp/ai/data/attachments";
const EXPORTS_REL_PATH: &str = ".acp/ai/data/exports";
const PREFS_REL_PATH: &str = ".acp/preferences.toml";

const FIREFOX_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0";

// Verified (from the public /auth page):
const EMAIL_EVENT: &str = "send-login-email";
const EMAIL_FIELD: &str = "user[email]";
const EMAIL_FORM_CID: u64 = 1; // phx-target="1" on #auth-page-email-form

// GUESSED (behind email step; auto-detected from the LiveView reply when possible):
const CODE_EVENT_FALLBACK: &str = "verify-code";
/// Verified from a real run: action of the phx-trigger-action form.
const OTP_SIGN_IN_FALLBACK: &str = "/auth/user/otp/sign_in";
const CODE_FIELD_FALLBACK: &str = "user[code]";

// Verified (logged-in inspection + Feed-*.js / Post-*.js bundles):
const PROFILE_URL: &str = "https://acp.us/rs/@{nick}";
const THREAD_URL: &str = "https://acp.us/rs/post/{id}";
const FEED_PROPS_SEL: &str = "div#forum\\/Feed[data-props]";
const THREAD_PROPS_SEL: &str = "div#forum\\/Thread[data-props]";
/// Found in Feed-*.js: `live.pushEvent("page-next", {cursor}, reply => ...)`,
/// reply = {posts, next_cursor, at_end}. No phx-target (sent to the root LiveView).
const FEED_NEXT_EVENT: &str = "page-next";
/// GUESSED: thread replies use the generic InfiniteScroll hook
/// (`pushEventTo(this.el, "next-page", {})`), target cid read from the page.
const THREAD_NEXT_EVENT: &str = "next-page";
const MAX_PAGES: u32 = 10_000;
/// Site-wide feeds. Verified by HTTP probes (auth redirect return_to):
///   /rs         → Trending (site default tab; /rs/trending is 404)
///   /rs/latest  → Latest
///   /rs/top     → Top
/// Profiles: /rs/@{nick} is Latest; /rs/@{nick}/top exists; no /rs/@{nick}/latest.
/// No /rs/search in the public JS (composer has mention-query only).
const FEED_TRENDING_URL: &str = "https://acp.us/rs";
const FEED_LATEST_URL: &str = "https://acp.us/rs/latest";
/// GUESSED (ForumProfileInfo hook): hover-card event, unused for scraping.
const PROFILE_SHOW_INFO_EVENT: &str = "show-info";
const DEFAULT_CRAWL_LIMIT: usize = 500;

// ---------------------------------------------------------------- prefs
#[derive(Serialize, Deserialize, Default)]
struct Prefs {
    email: Option<String>,
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| anyhow!("HOME not set"))
}

fn prompt(msg: &str) -> Result<String> {
    eprint!("{msg}");
    io::stderr().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

fn load_email() -> Result<String> {
    let path = home()?.join(PREFS_REL_PATH);
    if path.exists() {
        let p: Prefs = toml::from_str(&std::fs::read_to_string(&path)?)?;
        if let Some(e) = p.email.filter(|e| !e.is_empty()) {
            return Ok(e);
        }
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    let email = prompt("Email: ")?;
    std::fs::write(&path, toml::to_string(&Prefs { email: Some(email.clone()) })?)?;
    Ok(email)
}

// ---------------------------------------------------------------- http
fn client(jar: Arc<Jar>) -> Result<reqwest::Client> {
    use reqwest::header::*;
    let mut h = HeaderMap::new();
    h.insert(ACCEPT, HeaderValue::from_static(
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
    h.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
    h.insert("DNT", HeaderValue::from_static("1"));
    h.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    h.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
    h.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
    h.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    Ok(reqwest::Client::builder()
        .user_agent(FIREFOX_UA)
        .default_headers(h)
        .cookie_provider(jar)
        .build()?)
}

struct LvPage {
    url: String,
    csrf: String,
    id: String,
    session: String,
    statik: String,
}

fn parse_lv(url: &str, html: &str) -> Result<LvPage> {
    let doc = Html::parse_document(html);
    let csrf = doc
        .select(&Selector::parse("meta[name=csrf-token]").unwrap())
        .next()
        .and_then(|m| m.value().attr("content"))
        .context("no csrf-token")?;
    let main = doc
        .select(&Selector::parse("[data-phx-main]").unwrap())
        .next()
        .context("no LiveView root")?
        .value();
    Ok(LvPage {
        url: url.into(),
        csrf: csrf.into(),
        id: main.attr("id").unwrap_or_default().into(),
        session: main.attr("data-phx-session").unwrap_or_default().into(),
        statik: main.attr("data-phx-static").unwrap_or_default().into(),
    })
}

// ---------------------------------------------------------------- liveview diff rendering
/// Merge a LiveView diff into the rendered tree (LV rule: a subtree that
/// carries its own statics `s` replaces, otherwise merge key by key).
fn lv_merge(base: &mut Value, diff: &Value) {
    match (base.as_object_mut(), diff.as_object()) {
        (Some(b), Some(d)) => {
            for (k, v) in d {
                match b.get_mut(k) {
                    Some(old) if v.is_object() && v.get("s").is_none() && old.is_object() => lv_merge(old, v),
                    _ => { b.insert(k.clone(), v.clone()); }
                }
            }
        }
        _ => *base = diff.clone(),
    }
}

fn lv_statics<'a>(node: &'a Value, root: &'a Value, comps: &'a Value) -> Vec<String> {
    let strs = |v: &Value| v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect::<Vec<_>>());
    match &node["s"] {
        Value::Array(_) => strs(&node["s"]).unwrap_or_default(),
        Value::Number(n) => {
            let i = n.as_i64().unwrap_or(0);
            if i < 0 {
                // shared statics of another component
                lv_statics(&comps[(-i).to_string()], root, comps)
            } else {
                strs(&root["p"][i.to_string()]).unwrap_or_default()
            }
        }
        _ => vec![],
    }
}

fn lv_render(node: &Value, root: &Value, comps: &Value, out: &mut String) {
    match node {
        Value::String(s) => out.push_str(s),
        Value::Number(n) => {
            // component reference
            if let Some(c) = comps.get(n.to_string()) { lv_render(c, root, comps, out); }
        }
        Value::Object(_) => {
            let st = lv_statics(node, root, comps);
            if let Some(rows) = node.get("d").and_then(Value::as_array) {
                for row in rows {
                    for (i, s) in st.iter().enumerate() {
                        out.push_str(s);
                        if let Some(v) = row.get(i) { lv_render(v, root, comps, out); }
                    }
                }
            } else {
                for (i, s) in st.iter().enumerate() {
                    out.push_str(s);
                    if let Some(v) = node.get(i.to_string()) { lv_render(v, root, comps, out); }
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- liveview
static DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel { Quiet = 0, Normal = 1, Verbose = 2, Debug = 3 }

static LOG_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);

fn log_level() -> LogLevel {
    match LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) {
        0 => LogLevel::Quiet,
        2 => LogLevel::Verbose,
        3 => LogLevel::Debug,
        _ => LogLevel::Normal,
    }
}

fn set_log_level(l: LogLevel) {
    LOG_LEVEL.store(l as u8, std::sync::atomic::Ordering::Relaxed);
    DEBUG.store(l >= LogLevel::Debug, std::sync::atomic::Ordering::Relaxed);
}

/// Progress / summary (hidden by --quiet).
fn log_info(args: std::fmt::Arguments<'_>) {
    if log_level() >= LogLevel::Normal { eprintln!("{args}"); }
}
/// Extra detail (shown with --verbose / --debug).
fn log_verbose(args: std::fmt::Arguments<'_>) {
    if log_level() >= LogLevel::Verbose { eprintln!("{args}"); }
}
/// Always (errors / warnings users must see).
fn log_error(args: std::fmt::Arguments<'_>) {
    eprintln!("{args}");
}

macro_rules! info { ($($t:tt)*) => { $crate::log_info(format_args!($($t)*)) }; }
macro_rules! verbose { ($($t:tt)*) => { $crate::log_verbose(format_args!($($t)*)) }; }
macro_rules! err { ($($t:tt)*) => { $crate::log_error(format_args!($($t)*)) }; }

fn dbg_log(dir: &str, msg: &str) {
    if log_level() >= LogLevel::Debug {
        let cut: String = msg.chars().take(2000).collect();
        let more = if cut.len() < msg.len() { format!(" ...[{} bytes]", msg.len()) } else { String::new() };
        eprintln!("[debug {dir}] {cut}{more}");
    }
}

/// Remove reply/events/title keys before merging a diff into the tree.
fn strip_meta(d: &Value) -> Value {
    let mut d = d.clone();
    if let Some(m) = d.as_object_mut() { for k in ["r", "e", "t"] { m.remove(k); } }
    d
}

type Ws = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

struct Lv {
    ws: Ws,
    topic: String,
    r: u64,
    rendered: Value,
}

impl Lv {
    async fn connect(p: &LvPage, jar: &Jar) -> Result<Self> {
        let u = format!(
            "wss://acp.us/live/websocket?_csrf_token={}&vsn=2.0.0",
            urlenc(&p.csrf)
        );
        let mut req = u.into_client_request()?;
        let h = req.headers_mut();
        h.insert("User-Agent", FIREFOX_UA.parse()?);
        h.insert("Origin", BASE.parse()?);
        if let Some(c) = jar.cookies(&Url::parse(BASE)?) {
            h.insert("Cookie", c);
        }
        let (ws, _) = tokio_tungstenite::connect_async(req).await?;
        let mut lv = Lv { ws, topic: format!("lv:{}", p.id), r: 0, rendered: json!({}) };
        let reply = lv
            .push("phx_join", json!({
                "url": p.url,
                "params": {"_csrf_token": p.csrf, "_mounts": 0},
                "session": p.session,
                "static": p.statik,
            }))
            .await?;
        if reply["status"] != "ok" {
            bail!("LiveView join failed: {reply}");
        }
        lv.rendered = reply["response"]["rendered"].clone();
        Ok(lv)
    }

    async fn push(&mut self, event: &str, payload: Value) -> Result<Value> {
        self.r += 1;
        let r = self.r.to_string();
        let msg = json!(["1", r, self.topic, event, payload]);
        dbg_log("push", &msg.to_string());
        self.ws.send(Message::text(msg.to_string())).await?;
        while let Some(m) = self.ws.next().await {
            if let Message::Text(t) = m? {
                let v: Value = serde_json::from_str(&t)?;
                dbg_log("recv", &t);
                if v[3] == "diff" {
                    lv_merge(&mut self.rendered, &strip_meta(&v[4]));
                }
                if v[1] == r.as_str() && v[3] == "phx_reply" {
                    if let Some(d) = v[4].pointer("/response/diff") {
                        lv_merge(&mut self.rendered, &strip_meta(d));
                    }
                    return Ok(v[4].clone());
                }
            }
        }
        bail!("websocket closed")
    }

    /// Render the current LiveView tree (root plus every component) to HTML.
    fn html(&self) -> String {
        let mut out = String::new();
        let comps = self.rendered.get("c").cloned().unwrap_or(json!({}));
        lv_render(&self.rendered, &self.rendered, &comps, &mut out);
        if let Some(m) = comps.as_object() {
            for c in m.values() { lv_render(c, &self.rendered, &comps, &mut out); }
        }
        out
    }

    async fn form(&mut self, event: &str, field: &str, val: &str, cid: Option<u64>) -> Result<Value> {
        let mut p = json!({
            "type": "form",
            "event": event,
            "value": format!("{}={}", urlenc(field), urlenc(val)),
        });
        if let Some(c) = cid {
            p["cid"] = json!(c);
        }
        self.push("event", p).await
    }
}

fn urlenc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Look for a redirect target anywhere in a LiveView reply.
fn find_redirect(v: &Value) -> Option<String> {
    for k in ["redirect", "live_redirect", "navigate"] {
        if let Some(to) = v.pointer(&format!("/response/{k}/to")).and_then(Value::as_str) {
            return Some(to.into());
        }
    }
    None
}

/// Discover the code form's event / field / cid from the rendered LiveView
/// (falls back to the raw reply text, then to constants).
fn detect_code_form(html: &str, reply: &Value) -> (String, String, Option<u64>) {
    let raw = format!("{html}{}", reply.to_string().replace("\\\"", "\""));
    let ev = regex::Regex::new(r#"phx-submit="([^"]+)""#).unwrap()
        .captures_iter(&raw).map(|c| c[1].to_string())
        .find(|e| e != EMAIL_EVENT)
        .unwrap_or_else(|| CODE_EVENT_FALLBACK.into());
    let doc = Html::parse_fragment(html);
    let field = doc.select(&Selector::parse("form input").unwrap())
        .filter(|i| !matches!(i.value().attr("type"), Some("hidden" | "email")))
        .filter_map(|i| i.value().attr("name").map(str::to_string))
        .find(|n| n != EMAIL_FIELD && !n.starts_with('_'))
        .unwrap_or_else(|| CODE_FIELD_FALLBACK.into());
    let cid = Selector::parse(&format!("form[phx-submit='{ev}']")).ok()
        .and_then(|sel| doc.select(&sel).next()
            .and_then(|f| f.value().attr("phx-target")?.parse().ok()))
        .or(Some(EMAIL_FORM_CID));
    (ev, field, cid)
}

/// `phx-trigger-action`: LiveView asks the client to do a real HTTP POST of
/// the form (action="/auth/user/otp/sign_in") so the server can set the
/// authenticated session cookie.
async fn trigger_action_post(
    http: &reqwest::Client, html: &str, csrf: &str, email: &str, code_field: &str, code: &str,
) -> Result<()> {
    let doc = Html::parse_fragment(html);
    let form = doc.select(&Selector::parse("form[phx-trigger-action]").unwrap()).next()
        .ok_or_else(|| anyhow!("no redirect and no phx-trigger-action form after code"))?;
    let action = form.value().attr("action").unwrap_or(OTP_SIGN_IN_FALLBACK);
    let mut fields: Vec<(String, String)> = Vec::new();
    for i in form.select(&Selector::parse("input[name]").unwrap()) {
        let name = i.value().attr("name").unwrap_or_default().to_string();
        let mut val = i.value().attr("value").unwrap_or_default().to_string();
        if val.is_empty() {
            if name == EMAIL_FIELD { val = email.into(); }
            else if name == code_field { val = code.into(); }
        }
        fields.push((name, val));
    }
    if !fields.iter().any(|(n, _)| n == "_csrf_token") {
        fields.push(("_csrf_token".into(), csrf.into()));
    }
    let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
    verbose!("POST {action} with fields {names:?}");
    let resp = http.post(Url::parse(BASE)?.join(action)?)
        .header("Origin", BASE)
        .header("Referer", format!("{BASE}/auth"))
        .header("Sec-Fetch-Mode", "navigate")
        .form(&fields)
        .send().await?;
    if !resp.status().is_success() {
        bail!("sign-in POST failed: {}", resp.status());
    }
    Ok(())
}

async fn login(http: &reqwest::Client, jar: &Arc<Jar>) -> Result<()> {
    let resp = http.get(START_URL).send().await?;
    let url = resp.url().to_string();
    let html = resp.text().await?;
    if !url.contains("/auth") {
        info!("Already logged in.");
        return Ok(());
    }
    let page = parse_lv(&url, &html)?;
    let email = load_email()?;
    let mut lv = Lv::connect(&page, jar).await?;
    let r = lv.form(EMAIL_EVENT, EMAIL_FIELD, &email, Some(EMAIL_FORM_CID)).await?;
    if r["status"] != "ok" {
        bail!("email submit failed: {r}");
    }
    info!("Login email requested for {email}.");
    let code = prompt("Verification code (or paste the login link): ")?;
    if code.starts_with("http") {
        http.get(&code).send().await?.error_for_status()?;
    } else {
        let (ev, field, cid) = detect_code_form(&lv.html(), &r);
        let r2 = lv.form(&ev, &field, &code, cid).await?;
        if let Some(to) = find_redirect(&r2) {
            http.get(Url::parse(BASE)?.join(&to)?).send().await?.error_for_status()?;
        } else {
            trigger_action_post(http, &lv.html(), &page.csrf, &email, &field, &code).await?;
        }
    }
    let check = http.get(START_URL).send().await?;
    if check.url().path().starts_with("/auth") {
        bail!("still not logged in after verification");
    }
    info!("Logged in.");
    Ok(())
}

// ---------------------------------------------------------------- scrape
#[derive(Debug, Clone)]
pub struct Post {
    post_id: String,
    parent_post_id: String,
    nick: String,
    title: String,
    content: String,
    posted_at: String,
    /// Media referenced by the post (url, kind).
    media: Vec<MediaRef>,
}

#[derive(Debug, Clone)]
pub struct MediaRef {
    pub url: String,
    pub kind: String,
}

/// Verified (PostMedia-*.js): `post.media` = [{id, type, url, thumb_url, width, height}],
/// `type == "video"` renders `<video src=url poster=thumb_url>`; other types render `<img>`.
/// Body (ProseMirror) has no image node in the editor bundle; nodes found: embed
/// {provider,id,kind} (third-party iframes, not downloaded), postRef, mention.
/// GUESSED: any other node attr named src/url/href pointing at the site's media hosts.
fn collect_media(v: &Value, out: &mut Vec<MediaRef>) {
    if let Some(ms) = v.get("media").and_then(Value::as_array) {
        for m in ms {
            let kind = m.get("type").and_then(Value::as_str).unwrap_or("image").to_string();
            if let Some(u) = m.get("url").and_then(Value::as_str) {
                out.push(MediaRef { url: u.into(), kind: kind.clone() });
            }
            if let Some(u) = m.get("thumb_url").and_then(Value::as_str) {
                out.push(MediaRef { url: u.into(), kind: "thumbnail".into() });
            }
        }
    }
    for k in ["attachments", "files", "images"] {
        if let Some(ms) = v.get(k).and_then(Value::as_array) {
            for m in ms {
                if let Some(u) = m.get("url").or(m.get("src")).and_then(Value::as_str) {
                    let kind = m.get("type").and_then(Value::as_str).unwrap_or("file").to_string();
                    out.push(MediaRef { url: u.into(), kind });
                }
            }
        }
    }
    fn walk(n: &Value, out: &mut Vec<MediaRef>) {
        if let Some(a) = n.get("attrs").and_then(Value::as_object) {
            for k in ["src", "url", "href"] {
                if let Some(u) = a.get(k).and_then(Value::as_str)
                    && MEDIA_HOSTS.iter().any(|h| u.contains(h))
                {
                    let kind = n.get("type").and_then(Value::as_str).unwrap_or("file").to_string();
                    out.push(MediaRef { url: u.into(), kind });
                }
            }
        }
        for key in ["content", "marks"] {
            if let Some(kids) = n.get(key).and_then(Value::as_array) { kids.iter().for_each(|k| walk(k, out)); }
        }
    }
    if let Some(b) = v.get("body") {
        let b = match b.as_str() { Some(r) => serde_json::from_str(r).unwrap_or(Value::Null), None => b.clone() };
        walk(&b, out);
    }
}

/// Verified from the site CSP img-src/media-src.
const MEDIA_HOSTS: &[&str] = &["media.acp.us", "posts-media.acp.us", "backblazeb2.com"];

/// Flatten a ProseMirror doc (`{type, text?, content:[...]}`) to plain text.
fn prosemirror_text(v: &Value, out: &mut String) {
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        out.push_str(t);
    }
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    if ty == "hard_break" || ty == "hardBreak" {
        out.push('\n');
    }
    if let Some(kids) = v.get("content").and_then(Value::as_array) {
        for k in kids {
            prosemirror_text(k, out);
        }
    }
    if matches!(ty, "paragraph" | "heading" | "blockquote" | "list_item" | "listItem" | "code_block" | "codeBlock") {
        out.push('\n');
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn to_post(v: &Value, parent: &str, fallback_nick: &str) -> Post {
    let mut content = String::new();
    if let Some(b) = v.get("body") {
        let b = match b.as_str() {
            Some(raw) => serde_json::from_str(raw).unwrap_or(Value::Null),
            None => b.clone(),
        };
        prosemirror_text(&b, &mut content);
    }
    let mut content = content.trim().to_string();
    if content.is_empty() {
        content = s(v, "body_text_summary");
    }
    let nick = v.pointer("/author/username").and_then(Value::as_str)
        .map(str::to_string).unwrap_or_else(|| fallback_nick.to_string());
    Post {
        post_id: s(v, "id"),
        parent_post_id: parent.to_string(),
        nick,
        title: s(v, "title"),
        content,
        posted_at: s(v, "inserted_at"),
        media: { let mut m = Vec::new(); collect_media(v, &mut m); m },
    }
}

fn props(html: &str, sel: &str) -> Option<Value> {
    let doc = Html::parse_document(html);
    let el = doc.select(&Selector::parse(sel).ok()?).next()?;
    serde_json::from_str(el.value().attr("data-props")?).ok()
}

/// LiveView put `{:reply, map}` under response.r (LV >= 0.18) or response itself.
/// Verified in bundled phoenix_live_view 1.2.12: a hook pushEvent reply is
/// `phx_reply {status:"ok", response:{diff:{..., "r": <reply>}}}`
/// (Rendered.extract pops keys "r" reply, "e" events, "t" title from the diff).
fn reply_body(v: &Value) -> &Value {
    let r = &v["response"];
    if let Some(x) = r.pointer("/diff/r") { x } else if let Some(x) = r.get("r") { x } else { r }
}

/// Collect every object having an `id` and `inserted_at` from any `replies`
/// array in a JSON value, including JSON hidden in strings (diffs / data-props).
fn harvest_replies(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::Array(rs)) = m.get("replies") {
                out.extend(rs.iter().filter(|r| r.get("inserted_at").is_some()).cloned());
            }
            m.values().for_each(|x| harvest_replies(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| harvest_replies(x, out)),
        Value::String(st) if st.contains("replies") => {
            let st = st.replace("&quot;", "\"").replace("&amp;", "&");
            if let Some(i) = st.find('{')
                && let Ok(j) = serde_json::from_str::<Value>(&st[i..st.rfind('}').unwrap_or(i) + 1]) {
                    harvest_replies(&j, out);
                }
        }
        _ => {}
    }
}

/// One feed (profile, /rs, tag...) fully paged via `page-next {cursor}`.
pub struct FeedPage {
    pub items: Vec<Value>,
    pub html: String,
    pub stopped_incremental: bool,
    pub pages: u32,
}

/// True when every id in `page` is already known (and the page is non-empty).
pub fn page_all_known(page: &[Value], known: &HashSet<String>) -> bool {
    !page.is_empty() && page.iter().all(|x| {
        let id = s(x, "id");
        !id.is_empty() && known.contains(&id)
    })
}

async fn fetch_feed(
    http: &reqwest::Client, jar: &Jar, url: &str, limit: Option<usize>,
    known: Option<&HashSet<String>>,
    allow_incremental: bool, // false for Trending: no early stop on all-known pages
) -> Result<FeedPage> {
    let resp = http.get(url).send().await?.error_for_status()?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let feed = props(&html, FEED_PROPS_SEL).with_context(|| format!("no forum/Feed data-props on {url}"))?;
    let mut items: Vec<Value> = feed["posts"].as_array().cloned().unwrap_or_default();
    let mut at_end = feed["at_end"].as_bool().unwrap_or(true);
    let cursor = feed["next_cursor"].as_str().map(str::to_string);
    let hit_limit = |n: usize| limit.is_some_and(|l| n >= l);
    let mut stopped_incremental = false;
    let mut pages: u32 = 1;
    info!("feed {url}: {} posts from initial page", items.len());
    if allow_incremental
        && let Some(k) = known
        && page_all_known(&items, k) {
            info!("feed: initial page all known — stopping (use --full to re-crawl)");
            stopped_incremental = true;
            at_end = true;
        }

    if !at_end && cursor.is_some() && !hit_limit(items.len()) && !stopped_incremental {
        let page = parse_lv(&final_url, &html)?;
        let mut lv = Lv::connect(&page, jar).await?;
        let (stop, extra) = page_rest(&mut lv, &mut items, cursor, limit, known, allow_incremental).await?;
        stopped_incremental = stop;
        pages += extra;
    }
    if let Some(l) = limit { items.truncate(l); }
    Ok(FeedPage { items, html, stopped_incremental, pages })
}

/// Page a feed that is open on `lv` to its end with `page-next {cursor}`,
/// appending new (id-deduped) posts to `items`. Returns
/// `(stopped_incremental, extra_pages)`. Shared by profile, site and
/// "Posts & replies" feeds so incremental/limit rules are identical.
async fn page_rest(
    lv: &mut Lv, items: &mut Vec<Value>, mut cursor: Option<String>, limit: Option<usize>,
    known: Option<&HashSet<String>>, allow_incremental: bool,
) -> Result<(bool, u32)> {
    let hit_limit = |n: usize| limit.is_some_and(|l| n >= l);
    let mut ids: HashSet<String> = items.iter().map(|x| s(x, "id")).collect();
    let mut stale = 0;
    let mut pages = 0;
    let mut at_end = false;
    let mut stopped_incremental = false;
    for _ in 0..MAX_PAGES {
        let Some(c) = cursor.clone() else { at_end = true; break };
        let r = lv.push("event", json!({"type": "hook", "event": FEED_NEXT_EVENT, "value": {"cursor": c}})).await?;
        if r["status"] != "ok" { err!("warning: page-next failed: {r}"); break; }
        let body = reply_body(&r);
        let body = if body["posts"].is_array() { body.clone() }
            else { props(&lv.html(), FEED_PROPS_SEL).unwrap_or(Value::Null) };
        let ps = body["posts"].as_array().cloned().unwrap_or_default();
        pages += 1;
        if allow_incremental
            && let Some(k) = known
            && page_all_known(&ps, k) {
                info!("feed: page {} all known — stopping (use --full to re-crawl)", pages + 1);
                stopped_incremental = true;
                break;
            }
        let new: Vec<Value> = ps.into_iter().filter(|x| ids.insert(s(x, "id"))).collect();
        verbose!("feed: +{} new posts", new.len());
        items.extend(new.iter().cloned());
        at_end = body["at_end"].as_bool().unwrap_or(new.is_empty());
        cursor = body["next_cursor"].as_str().map(str::to_string).or(cursor);
        if new.is_empty() { stale += 1; } else { stale = 0; }
        if hit_limit(items.len()) { info!("feed: reached limit {}", limit.unwrap_or(0)); break; }
        if at_end || stale >= 2 || (new.is_empty() && cursor.as_deref() == Some(c.as_str())) { break; }
    }
    if !at_end && !hit_limit(items.len()) && !stopped_incremental {
        err!("warning: feed paging stopped before at_end (run with --debug)");
    }
    Ok((stopped_incremental, pages))
}

/// How the profile's "Posts & replies" tab switches the feed, as found in the
/// rendered profile HTML (the tab is server-rendered and only visible when
/// logged in, so it is discovered at runtime rather than hard-coded).
#[derive(Debug, Clone, PartialEq)]
enum RepliesTab {
    /// A `live_patch`/`navigate`/plain link: load this URL as its own feed.
    Link(String),
    /// A `phx-click` event pushed on the profile LiveView, with its
    /// `phx-value-*` params (or JS `push` value) and optional `phx-target` cid.
    Event { event: String, value: serde_json::Map<String, Value>, cid: Option<u64> },
}

/// True for a tab label like "Posts & replies" / "Posts and replies".
fn is_replies_label(text: &str) -> bool {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let t = t.replace("&amp;", "&");
    t.contains("replies") && t.contains("posts") && (t.contains('&') || t.contains(" and "))
}

/// Find the "Posts & replies" tab on a profile page. Matches by visible label,
/// falling back to attributes that name a replies tab (`phx-value-tab=replies`,
/// `href` ending in `/replies` or containing `tab=replies`).
fn find_replies_tab(html: &str, page_url: &str) -> Option<RepliesTab> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("a, button, [phx-click], [role=tab]").ok()?;
    let attr_says_replies = |e: &scraper::node::Element| {
        e.attrs().any(|(k, v)| {
            let v = v.to_lowercase();
            (k.starts_with("phx-value-") && v.contains("repl"))
                || (k == "href" && (v.ends_with("/replies") || v.contains("tab=replies") || v.contains("with_replies")))
        })
    };
    let el = doc.select(&sel)
        .filter(|e| { let v = e.value(); v.attr("href").is_some() || v.attr("phx-click").is_some() })
        .find(|e| is_replies_label(&e.text().collect::<String>()))
        .or_else(|| doc.select(&sel).find(|e| attr_says_replies(e.value())))?;
    let v = el.value();
    let abs = |h: &str| Url::parse(page_url).ok().and_then(|b| b.join(h).ok()).map(|u| u.to_string());
    let cid = v.attr("phx-target").and_then(|t| t.parse::<u64>().ok());
    if let Some(click) = v.attr("phx-click") {
        let click = click.trim();
        if click.starts_with('[') {
            // Phoenix.LiveView.JS command list, e.g. [["push",{"event":"tab","value":{...}}]]
            let cmds: Value = serde_json::from_str(click).ok()?;
            for c in cmds.as_array()? {
                let (op, args) = (c[0].as_str().unwrap_or(""), &c[1]);
                match op {
                    "push" => {
                        let value = args["value"].as_object().cloned().unwrap_or_default();
                        let cid = args["target"].as_u64().or(cid);
                        return Some(RepliesTab::Event { event: args["event"].as_str()?.to_string(), value, cid });
                    }
                    "patch" | "navigate" => return abs(args["href"].as_str()?).map(RepliesTab::Link),
                    _ => {}
                }
            }
        } else if !click.is_empty() {
            let value = v.attrs()
                .filter_map(|(k, val)| k.strip_prefix("phx-value-").map(|k| (k.to_string(), json!(val))))
                .collect();
            return Some(RepliesTab::Event { event: click.to_string(), value, cid });
        }
    }
    v.attr("href").filter(|h| !h.is_empty() && *h != "#").and_then(abs).map(RepliesTab::Link)
}

/// Fetch the profile's "Posts & replies" feed (paged to the end), using the
/// tab found by [`find_replies_tab`]. `None` when the profile has no such tab.
async fn fetch_replies_feed(
    http: &reqwest::Client, jar: &Jar, profile_url: &str, profile_html: &str,
    known: Option<&HashSet<String>>,
) -> Result<Option<Vec<Value>>> {
    let Some(tab) = find_replies_tab(profile_html, profile_url) else {
        verbose!("profile: no 'Posts & replies' tab found");
        return Ok(None);
    };
    dbg_log("profile", &format!("posts & replies tab: {tab:?}"));
    match tab {
        RepliesTab::Link(url) => {
            verbose!("profile: 'Posts & replies' tab -> {url}");
            Ok(Some(fetch_feed(http, jar, &url, None, known, true).await?.items))
        }
        RepliesTab::Event { event, value, cid } => {
            verbose!("profile: 'Posts & replies' tab -> phx-click {event} {value:?}");
            let page = parse_lv(profile_url, profile_html)?;
            let mut lv = Lv::connect(&page, jar).await?;
            let mut ev = json!({"type": "click", "event": event, "value": Value::Object(value)});
            if let Some(c) = cid { ev["cid"] = json!(c); }
            let r = lv.push("event", ev).await?;
            if r["status"] != "ok" { bail!("posts & replies tab event failed: {r}"); }
            let body = reply_body(&r);
            let feed = if body["posts"].is_array() { body.clone() }
                else { props(&lv.html(), FEED_PROPS_SEL).context("no forum/Feed props after tab switch")? };
            let mut items: Vec<Value> = feed["posts"].as_array().cloned().unwrap_or_default();
            info!("feed posts & replies: {} posts from initial page", items.len());
            let at_end = feed["at_end"].as_bool().unwrap_or(true);
            let cursor = feed["next_cursor"].as_str().map(str::to_string);
            if known.is_some_and(|k| page_all_known(&items, k)) {
                info!("feed: initial page all known — stopping (use --full to re-crawl)");
            } else if !at_end && cursor.is_some() {
                page_rest(&mut lv, &mut items, cursor, None, known, true).await?;
            }
            Ok(Some(items))
        }
    }
}

/// Append `extra` feed items whose `id` isn't already in `items`.
/// Returns how many were added.
fn merge_feed_items(items: &mut Vec<Value>, extra: Vec<Value>) -> usize {
    let mut ids: HashSet<String> = items.iter().map(|x| s(x, "id")).collect();
    let before = items.len();
    items.extend(extra.into_iter().filter(|x| { let id = s(x, "id"); !id.is_empty() && ids.insert(id) }));
    items.len() - before
}

/// Thread page props plus all replies (paged via the InfiniteScroll hook).
async fn fetch_thread(http: &reqwest::Client, jar: &Jar, tid: &str) -> Result<(Value, Vec<Value>)> {
    let url = THREAD_URL.replace("{id}", tid);
    let resp = http.get(&url).send().await?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let th = props(&html, THREAD_PROPS_SEL).with_context(|| format!("no forum/Thread props for {tid}"))?;
    let mut replies: Vec<Value> = th["replies"].as_array().cloned().unwrap_or_default();
    // Thread.js has no load-more event; more replies come from the
    // InfiniteScroll hook (`pushEventTo(this.el, "next-page", {})`),
    // whose target cid is the element's phx-target (none = root view).
    // New replies arrive as a diff re-rendering forum/Thread data-props.
    let (hook, cid) = {
        let doc = Html::parse_document(&html);
        let sel = Selector::parse("[phx-hook=InfiniteScroll],[data-phx-hook=InfiniteScroll]").unwrap();
        let el = doc.select(&sel).next();
        (el.is_some(), el.and_then(|e| e.value().attr("phx-target").and_then(|t| t.parse::<u64>().ok())))
    };
    let total = th.pointer("/root/replies_count").and_then(Value::as_u64).unwrap_or(0) as usize;
    if (hook || total > replies.len())
        && let Ok(page) = parse_lv(&final_url, &html)
        && let Ok(mut lv) = Lv::connect(&page, jar).await
    {
        let mut ids: HashSet<String> = replies.iter().map(|x| s(x, "id")).collect();
        for _ in 0..MAX_PAGES {
            let mut ev = json!({"type": "hook", "event": THREAD_NEXT_EVENT, "value": {}});
            if let Some(c) = cid { ev["cid"] = json!(c); }
            let Ok(r) = lv.push("event", ev).await else { break };
            let mut got = Vec::new();
            harvest_replies(reply_body(&r), &mut got);
            if let Some(p) = props(&lv.html(), THREAD_PROPS_SEL) { harvest_replies(&p, &mut got); }
            let new: Vec<Value> = got.into_iter().filter(|x| ids.insert(s(x, "id"))).collect();
            if new.is_empty() { break; }
            replies.extend(new);
        }
    }
    Ok((th, replies))
}

/// Scrape a nick: their feed (root posts + their replies elsewhere) and all
/// replies in their own threads. Also returns the profile page HTML.
#[derive(Default)]
struct ScrapeStats {
    feed_items: usize,
    new_posts: usize,
    skipped_known: usize,
    threads_fetched: usize,
    threads_skipped: usize,
    /// Items only found via the profile's "Posts & replies" tab.
    replies_tab_items: usize,
}

async fn scrape(
    http: &reqwest::Client, jar: &Jar, nick: &str,
    known: &HashSet<String>, reply_counts: &HashMap<String, usize>, full: bool,
) -> Result<(Vec<Post>, String, ScrapeStats)> {
    let nick = nick.trim_start_matches('@');
    let known_opt = if full { None } else { Some(known) };
    // Profile feed is always Latest (/rs/@nick); Trending does not apply to profiles.
    let profile_url = PROFILE_URL.replace("{nick}", nick);
    let mut feed = fetch_feed(http, jar, &profile_url, None, known_opt, true).await?;
    // Also walk the "Posts & replies" tab (the nick's replies in other threads).
    let mut replies_tab_added = 0;
    match fetch_replies_feed(http, jar, &profile_url, &feed.html, known_opt).await {
        Ok(Some(extra)) => {
            replies_tab_added = merge_feed_items(&mut feed.items, extra);
            info!("@{nick}: posts & replies tab added {replies_tab_added} items");
        }
        Ok(None) => {}
        Err(e) => err!("warning: posts & replies tab: {e:#}"),
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut roots: Vec<(String, usize)> = Vec::new(); // (id, replies_count from feed)
    let mut stats = ScrapeStats { feed_items: feed.items.len(), replies_tab_items: replies_tab_added, ..Default::default() };
    for it in &feed.items {
        let id = s(it, "id");
        if id.is_empty() || !seen.insert(id.clone()) { continue; }
        let thread = s(it, "thread_id");
        let is_root = thread.is_empty() || thread == id;
        let already = known.contains(&id);
        if already && !full {
            stats.skipped_known += 1;
        } else {
            stats.new_posts += 1;
        }
        if is_root {
            let rc = it.get("replies_count").and_then(Value::as_u64).unwrap_or(0) as usize;
            roots.push((id.clone(), rc));
            if !already || full {
                out.push(to_post(it, "", nick));
            }
        } else if !already || full {
            out.push(to_post(it, &thread, nick));
        }
    }
    for (tid, feed_rc) in roots {
        let db_rc = reply_counts.get(&tid).copied().unwrap_or(0);
        let need = full || !known.contains(&tid) || feed_rc > db_rc;
        if !need {
            stats.threads_skipped += 1;
            continue;
        }
        stats.threads_fetched += 1;
        let (_, replies) = match fetch_thread(http, jar, &tid).await {
            Ok(x) => x,
            Err(e) => { err!("warning: {e:#}"); continue; }
        };
        let mut n = 0;
        for rp in &replies {
            let id = s(rp, "id");
            if id.is_empty() || id == tid || !seen.insert(id.clone()) { continue; }
            if known.contains(&id) && !full {
                stats.skipped_known += 1;
                continue;
            }
            out.push(to_post(rp, &tid, nick));
            stats.new_posts += 1;
            n += 1;
        }
        verbose!("thread {tid}: {n} new replies (feed says {feed_rc}, db had {db_rc})");
    }
    Ok((out, feed.html, stats))
}

// ---------------------------------------------------------------- db
fn db_known_ids(db: &std::path::Path) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    if !db.exists() { return Ok(out); }
    let c = Connection::open(db)?;
    let ok: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !ok { return Ok(out); }
    let mut st = c.prepare("SELECT post_id FROM posts")?;
    for id in st.query_map([], |r| r.get::<_, String>(0))? { out.insert(id?); }
    Ok(out)
}

fn db_reply_counts(db: &std::path::Path) -> Result<HashMap<String, usize>> {
    let mut out = HashMap::new();
    if !db.exists() { return Ok(out); }
    let c = Connection::open(db)?;
    let ok: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !ok { return Ok(out); }
    let mut st = c.prepare(
        "SELECT parent_post_id, COUNT(*) FROM posts WHERE parent_post_id != '' GROUP BY parent_post_id")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))? {
        let (k, v) = row?;
        out.insert(k, v);
    }
    Ok(out)
}

#[derive(Debug)]
struct NickInfo {
    nick: String,
    display_name: Option<String>,
    post_count: i64,
    latest: Option<String>,
    profile_scraped: bool,
    scraped_at: Option<String>,
}

fn list_nicks_db(db: &std::path::Path) -> Result<Vec<NickInfo>> {
    let mut map: HashMap<String, NickInfo> = HashMap::new();
    if !db.exists() { return Ok(Vec::new()); }
    let c = Connection::open(db)?;
    let has_posts: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if has_posts {
        let mut st = c.prepare(
            "SELECT nick, COUNT(*), MAX(posted_at) FROM posts GROUP BY lower(nick)")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (nick, cnt, latest) = row?;
            let key = nick.to_lowercase();
            map.insert(key, NickInfo {
                nick, display_name: None, post_count: cnt, latest,
                profile_scraped: false, scraped_at: None,
            });
        }
    }
    let has_p: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='profiles'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if has_p {
        let mut st = c.prepare("SELECT nick, display_name, scraped_at FROM profiles")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (nick, dn, sa) = row?;
            let key = nick.to_lowercase();
            let e = map.entry(key).or_insert_with(|| NickInfo {
                nick: nick.clone(), display_name: None, post_count: 0, latest: None,
                profile_scraped: false, scraped_at: None,
            });
            e.nick = nick;
            e.display_name = dn.or(e.display_name.clone());
            e.profile_scraped = true;
            e.scraped_at = sa;
        }
    }
    let mut v: Vec<_> = map.into_values().collect();
    v.sort_by_key(|a| a.nick.to_lowercase());
    Ok(v)
}

fn print_nicks_table(infos: &[NickInfo]) {
    println!("{:<20} {:<24} {:>6}  {:<20} {:<8} SCRAPED_AT",
        "NICK", "DISPLAY", "POSTS", "LATEST", "PROFILE");
    for i in infos {
        println!("{:<20} {:<24} {:>6}  {:<20} {:<8} {}",
            format!("@{}", i.nick),
            i.display_name.as_deref().unwrap_or("-").chars().take(24).collect::<String>(),
            i.post_count,
            i.latest.as_deref().unwrap_or("-").chars().take(20).collect::<String>(),
            if i.profile_scraped { "yes" } else { "no" },
            i.scraped_at.as_deref().unwrap_or("-"));
    }
}

fn save(posts: &[Post]) -> Result<usize> {
    let path = home()?.join(DB_REL_PATH);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut db = Connection::open(&path)?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL,
            nick TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            posted_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS posts_post_id_uq ON posts(post_id);",
    )?;
    let tx = db.transaction()?;
    let mut n = 0;
    {
        let mut st = tx.prepare(
            "INSERT OR IGNORE INTO posts (post_id,parent_post_id,nick,title,content,posted_at)
             VALUES (?1,?2,?3,?4,?5,?6)")?;
        for p in posts {
            n += st.execute(params![p.post_id, p.parent_post_id, p.nick, p.title, p.content, p.posted_at])?;
        }
    }
    tx.commit()?;
    info!("DB: {}", path.display());
    Ok(n)
}

fn save_profile(db: &std::path::Path, p: &mentions::Profile) -> Result<()> {
    let c = Connection::open(db)?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS profiles (
            nick TEXT PRIMARY KEY,
            user_id TEXT,
            display_name TEXT,
            bio TEXT,
            avatar_url TEXT,
            avatar_path TEXT,
            banner_url TEXT,
            banner_path TEXT,
            location TEXT,
            role TEXT,
            joined_at TEXT,
            followers INTEGER,
            following INTEGER,
            posts_count INTEGER,
            raw_json TEXT NOT NULL,
            scraped_at TEXT NOT NULL
        );")?;
    c.execute(
        "INSERT INTO profiles (nick,user_id,display_name,bio,avatar_url,avatar_path,banner_url,banner_path,
            location,role,joined_at,followers,following,posts_count,raw_json,scraped_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
         ON CONFLICT(nick) DO UPDATE SET
            user_id=excluded.user_id, display_name=excluded.display_name, bio=excluded.bio,
            avatar_url=excluded.avatar_url, avatar_path=excluded.avatar_path,
            banner_url=excluded.banner_url, banner_path=excluded.banner_path,
            location=excluded.location, role=excluded.role, joined_at=excluded.joined_at,
            followers=excluded.followers, following=excluded.following, posts_count=excluded.posts_count,
            raw_json=excluded.raw_json, scraped_at=excluded.scraped_at",
        rusqlite::params![
            p.nick, p.user_id, p.display_name, p.bio, p.avatar_url, p.avatar_path,
            p.banner_url, p.banner_path, p.location, p.role, p.joined_at,
            p.followers, p.following, p.posts_count, p.raw_json, p.scraped_at
        ],
    )?;
    Ok(())
}

fn save_mentions(db: &std::path::Path, hits: &[mentions::MentionHit]) -> Result<()> {
    let mut c = Connection::open(db)?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS mentions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            mentioned_nick TEXT NOT NULL,
            match_type TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS mentions_uq ON mentions(post_id, mentioned_nick, match_type);")?;
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT OR IGNORE INTO mentions (post_id, mentioned_nick, match_type) VALUES (?1,?2,?3)")?;
        for h in hits {
            st.execute(rusqlite::params![h.post_id, h.mentioned_nick, h.match_type.as_str()])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn load_profiles_mentions(db: &std::path::Path, nicks: &[String]) -> Result<(Vec<mentions::Profile>, Vec<mentions::MentionHit>)> {
    if !db.exists() { return Ok((Vec::new(), Vec::new())); }
    let c = Connection::open(db)?;
    let mut profiles = Vec::new();
    let has_p: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='profiles'", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) > 0;
    if has_p {
        let mut st = c.prepare("SELECT nick,user_id,display_name,bio,avatar_url,avatar_path,banner_url,banner_path,location,role,joined_at,followers,following,posts_count,raw_json,scraped_at FROM profiles")?;
        for row in st.query_map([], |r| Ok(mentions::Profile {
            nick: r.get(0)?, user_id: r.get(1)?, display_name: r.get(2)?, bio: r.get(3)?,
            avatar_url: r.get(4)?, avatar_path: r.get(5)?, banner_url: r.get(6)?, banner_path: r.get(7)?,
            location: r.get(8)?, role: r.get(9)?, joined_at: r.get(10)?, followers: r.get(11)?,
            following: r.get(12)?, posts_count: r.get(13)?, raw_json: r.get(14)?, scraped_at: r.get(15)?,
        }))? {
            let p = row?;
            if nicks.is_empty() || nicks.iter().any(|n| n.eq_ignore_ascii_case(&p.nick)) {
                profiles.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    let has_m: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='mentions'", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) > 0;
    if has_m {
        let mut st = c.prepare("SELECT post_id, mentioned_nick, match_type FROM mentions")?;
        for row in st.query_map([], |r| {
            let mt: String = r.get(2)?;
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, mt))
        })? {
            let (post_id, mentioned_nick, mt) = row?;
            if !nicks.is_empty() && !nicks.iter().any(|n| n.eq_ignore_ascii_case(&mentioned_nick)) { continue; }
            if let Some(match_type) = mentions::MatchType::parse(&mt) {
                hits.push(mentions::MentionHit { post_id, mentioned_nick, match_type });
            }
        }
    }
    Ok((profiles, hits))
}

#[allow(clippy::too_many_arguments)]
async fn crawl_mentions(
    http: &reqwest::Client, jar: &Jar, targets: &[mentions::MentionTarget],
    limit: usize, seen: &mut HashSet<String>, known: &HashSet<String>, full: bool,
    mode: FeedMode,
) -> Result<(Vec<Post>, Vec<mentions::MentionHit>)> {
    let allow_inc = mode.allow_incremental_stop() && !full;
    let known_opt = if allow_inc { Some(known) } else { None };
    let url = mode.site_feed_url();
    info!("mentions feed: {url} (mode={mode})");
    let feed = fetch_feed(http, jar, url, Some(limit), known_opt, allow_inc).await?;
    let mut posts = Vec::new();
    let mut hits = Vec::new();
    let mut checked = HashSet::new();
    for it in &feed.items {
        let id = s(it, "id");
        let tid = { let t = s(it, "thread_id"); if t.is_empty() { id.clone() } else { t } };
        let mut any = false;
        for t in targets {
            if !mentions::match_post(it, t).is_empty() { any = true; break; }
        }
        if !any { continue; }
        if !checked.insert(tid.clone()) { continue; }
        let (th, replies) = match fetch_thread(http, jar, &tid).await {
            Ok(x) => x,
            Err(e) => { err!("warning: mention thread {tid}: {e:#}"); continue; }
        };
        let root = th.get("root").cloned().unwrap_or(th.clone());
        let mut thread_posts = vec![root];
        thread_posts.extend(replies);
        for tp in &thread_posts {
            let pid = s(tp, "id");
            if pid.is_empty() { continue; }
            for t in targets {
                for mt in mentions::match_post(tp, t) {
                    hits.push(mentions::MentionHit {
                        post_id: pid.clone(), mentioned_nick: t.nick.clone(), match_type: mt,
                    });
                }
            }
            if !seen.insert(pid.clone()) { continue; }
            let parent = {
                let thid = s(tp, "thread_id");
                if thid.is_empty() || thid == pid { String::new() } else { thid }
            };
            let author = tp.pointer("/author/username").and_then(Value::as_str).unwrap_or("");
            posts.push(to_post(tp, &parent, author));
        }
    }
    Ok((posts, hits))
}


mod export;
mod maint;
mod mentions;

#[derive(clap::Parser, Clone)]
#[command(
    name = "acp-rs-scraper",
    version,
    about = "Scrape Red Star (acp.us/rs) posts into SQLite and export them",
    long_about = "Log in to https://acp.us/rs as Firefox, scrape posts (and optionally mentions + profiles) for one or more nicks into a local SQLite database, download attachments, and export JSON/HTML — or work entirely offline with --search / --export / --import.",
    after_help = "Examples:
  acp-rs-scraper --nicks @haz,@m3talsmith --format html --compress --tar-gz
  acp-rs-scraper --nick alice --no-mentions --crawl-limit 200
  acp-rs-scraper --search \"general strike\" --format json
  acp-rs-scraper --export all.json
  acp-rs-scraper --export strike.tar.gz --search strike --compress --tar-gz --name strike
  acp-rs-scraper --import strike.tar.gz --compress --tar-gz
  acp-rs-scraper --import all.json
  ACP_DEBUG=1 acp-rs-scraper --nick alice --debug
  acp-rs-scraper --list-nicks
  acp-rs-scraper --list-nicks --format json
  acp-rs-scraper --nick alice --full
  \
acp-rs-scraper --nick alice -q
  \
acp-rs-scraper --nick alice -v
  \
acp-rs-scraper --nicks @haz --latest
  \
acp-rs-scraper --nicks @haz --trending --crawl-limit 200
",
)]
#[command(group(clap::ArgGroup::new("compress_format").multiple(false)))]
#[command(group(clap::ArgGroup::new("mode").multiple(false)))]
#[command(group(clap::ArgGroup::new("maint").args(["remove_nick", "clear_all"]).multiple(false)))]
struct Args {
    /// Site feed: Latest tab (/rs/latest). Default — even though the site opens on Trending.
    #[arg(long, group = "mode", help_heading = "Mode")]
    latest: bool,
    /// Site feed: Trending tab (/rs). Opt-in; incremental early-stop disabled (not chronological).
    #[arg(long, group = "mode", help_heading = "Mode")]
    trending: bool,

    /// Single nick (alias for --nicks). Prompted if neither is given.
    #[arg(long, help_heading = "Scraping", conflicts_with = "nicks")]
    nick: Option<String>,
    /// Comma-separated nicks, e.g. "@haz,@m3talsmith" (@ optional, trimmed, deduped).
    #[arg(long, value_parser = parse_nicks, value_name = "LIST", help_heading = "Scraping")]
    nicks: Option<NickList>,
    /// Find posts that mention each nick (default: on). Crawls the site feed (--latest/--trending); no server search in the JS.
    #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue, help_heading = "Scraping")]
    #[arg(long = "no-mentions", action = clap::ArgAction::SetFalse, help_heading = "Scraping",
          help = "Skip the mentions crawl")]
    mentions: bool,
    /// Max site-feed posts to scan when hunting mentions (default 500).
    #[arg(long, default_value_t = DEFAULT_CRAWL_LIMIT, help_heading = "Scraping")]
    crawl_limit: usize,
    /// Skip downloading media attachments.
    #[arg(long, help_heading = "Scraping")]
    no_attachments: bool,
    /// Force a full re-crawl (ignore already-known post_ids when paging / refetching threads).
    #[arg(long, help_heading = "Scraping")]
    full: bool,
    /// Offline: list known nicks from the DB (posts + profiles).
    #[arg(long, help_heading = "Search / Export")]
    list_nicks: bool,

    /// Search the local DB (title + content); exports matches + parent/children.
    #[arg(long, help_heading = "Search / Export")]
    search: Option<String>,
    /// Offline: dump the local DB (all / --nick(s) / --search) to FILE. Replaces --output for DB dumps.
    #[arg(long, value_name = "FILE", help_heading = "Search / Export", conflicts_with_all = ["output", "import"])]
    export: Option<PathBuf>,
    /// Export name: basename, archive name, folder inside archive, HTML title.
    #[arg(long, help_heading = "Search / Export")]
    name: Option<String>,

    /// Offline: import a previous export into the DB. Add --compress --tar-gz for archives.
    #[arg(long, value_name = "FILE", help_heading = "Import",
          conflicts_with_all = ["export", "search", "nick", "nicks", "output", "format", "name", "no_attachments", "mentions", "crawl_limit", "full", "list_nicks"])]
    import: Option<PathBuf>,

    /// Export format.
    #[arg(long, value_enum, default_value_t = export::Format::Json, help_heading = "Output")]
    format: export::Format,
    /// Output file (with --compress: the archive path). Default: stdout.
    #[arg(long, help_heading = "Output")]
    output: Option<PathBuf>,

    /// Compress the export; requires a format flag (e.g. --tar-gz).
    #[arg(long, requires = "compress_format", help_heading = "Compression")]
    compress: bool,
    /// Compression format: gzip-compressed tar (.tar.gz). Requires --compress.
    #[arg(long, group = "compress_format", requires = "compress", help_heading = "Compression")]
    tar_gz: bool,

    /// Offline: delete these nicks (comma-separated, @ optional) — their posts, replies in their threads, attachments, mentions and profiles.
    #[arg(long, value_parser = parse_nicks, value_name = "LIST", help_heading = "Maintenance",
          conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "no_attachments", "full", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    remove_nick: Option<NickList>,
    /// With --remove-nick: keep other people's replies in the removed nicks' threads (they become orphans).
    #[arg(long, requires = "remove_nick", conflicts_with = "clear_all", help_heading = "Maintenance")]
    keep_replies: bool,
    /// Offline: delete the whole database and the attachments directory (preferences.toml and exports are kept).
    #[arg(long, help_heading = "Maintenance", conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "no_attachments", "full", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    clear_all: bool,
    /// With --remove-nick / --clear-all: show what would be removed and change nothing.
    #[arg(long, requires = "maint", help_heading = "Maintenance")]
    dry_run: bool,
    /// With --remove-nick / --clear-all: skip the confirmation prompt (required when stdin is not a terminal).
    #[arg(short = 'y', long, requires = "maint", help_heading = "Maintenance")]
    yes: bool,

    /// Packaging helper: print a man page or shell completion script to stdout.
    #[arg(long, value_enum, value_name = "WHAT", hide = true, exclusive = true)]
    generate: Option<Generate>,

    /// Suppress progress/summary on stderr (prompts and errors still show).
    #[arg(short = 'q', long, help_heading = "Debug", conflicts_with_all = ["verbose", "debug"])]
    quiet: bool,
    /// Extra progress detail (per-page / per-thread / skipped items).
    #[arg(short = 'v', long, help_heading = "Debug", conflicts_with = "quiet")]
    verbose: bool,
    /// Raw LiveView wire log (implies --verbose). Also ACP_DEBUG=1.
    #[arg(long, help_heading = "Debug", conflicts_with = "quiet")]
    debug: bool,
}

/// Targets for the hidden `--generate` packaging flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Generate { Man, Bash, Zsh, Fish }

/// Write the man page or a completion script for [`Args`] to `out`.
fn generate(what: Generate, out: &mut dyn Write) -> Result<()> {
    use clap::CommandFactory;
    let mut cmd = Args::command();
    let name = cmd.get_name().to_string();
    match what {
        Generate::Man => clap_mangen::Man::new(cmd).render(out)?,
        Generate::Bash => clap_complete::generate(clap_complete::Shell::Bash, &mut cmd, name, out),
        Generate::Zsh => clap_complete::generate(clap_complete::Shell::Zsh, &mut cmd, name, out),
        Generate::Fish => clap_complete::generate(clap_complete::Shell::Fish, &mut cmd, name, out),
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeedMode { Latest, Trending }

impl FeedMode {
    fn site_feed_url(self) -> &'static str {
        match self {
            FeedMode::Latest => FEED_LATEST_URL,
            FeedMode::Trending => FEED_TRENDING_URL,
        }
    }
    fn allow_incremental_stop(self) -> bool {
        matches!(self, FeedMode::Latest)
    }
}

impl std::fmt::Display for FeedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedMode::Latest => write!(f, "latest"),
            FeedMode::Trending => write!(f, "trending"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NickList(pub Vec<String>);

/// "@a, b,,@A" -> ["a", "b"]: strip @, trim, drop empties, dedupe (case-insensitive, first wins).
fn parse_nicks(s: &str) -> std::result::Result<NickList, String> {
    let mut seen = HashSet::new();
    let v: Vec<String> = s.split(',')
        .map(|n| n.trim().trim_start_matches('@').trim().to_string())
        .filter(|n| !n.is_empty() && seen.insert(n.to_lowercase()))
        .collect();
    if v.is_empty() { return Err("expected at least one nick".into()); }
    if let Some(bad) = v.iter().find(|n| n.contains(char::is_whitespace) || n.contains('/')) {
        return Err(format!("invalid nick {bad:?}"));
    }
    Ok(NickList(v))
}

/// Default export name for a set of nicks.
fn nicks_name(nicks: &[String], stamp: &str) -> String {
    let j = nicks.join("+");
    if nicks.len() > 4 || j.len() > 40 { format!("multi-{stamp}") } else { format!("{j}-{stamp}") }
}

impl Args {
    /// --nick / --nicks merged (empty = none given).
    fn nick_list(&self) -> Result<Vec<String>> {
        if let Some(n) = &self.nicks { return Ok(n.0.clone()); }
        match &self.nick { Some(n) => Ok(parse_nicks(n).map_err(|e| anyhow!(e))?.0), None => Ok(Vec::new()) }
    }

    /// Selected archive format (clap guarantees --compress <=> exactly one format flag).
    fn compression(&self) -> Option<export::Compress> {
        if !self.compress { return None; }
        if self.tar_gz { Some(export::Compress::TarGz) } else { None }
    }
    fn want_mentions(&self) -> bool { self.mentions }
    /// Default is Latest (our default), even though the site lands on Trending.
    fn feed_mode(&self) -> FeedMode {
        if self.trending { FeedMode::Trending } else { FeedMode::Latest }
    }
}

/// `--remove-nick`: preview (rolled-back transaction), confirm, then commit.
fn run_remove_nick(args: &Args, nicks: &[String]) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let att = home()?.join(ATTACH_REL_PATH);
    let preview = maint::remove_nicks(&db, &att, nicks, args.keep_replies, false)?;
    let who = nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
    eprintln!("{}", preview.summary(&format!("remove {who} would delete")));
    if args.keep_replies { eprintln!("--keep-replies: other people's replies in their threads are kept"); }
    if args.dry_run {
        for f in &preview.files { eprintln!("  file: {}", f.display()); }
        eprintln!("dry run: nothing changed");
        return Ok(());
    }
    if preview.is_empty() {
        eprintln!("nothing to remove");
        return Ok(());
    }
    if !maint::confirm("Proceed?", false, args.yes)? {
        eprintln!("aborted: nothing changed");
        return Ok(());
    }
    let done = maint::remove_nicks(&db, &att, nicks, args.keep_replies, true)?;
    eprintln!("{}", done.summary("removed"));
    Ok(())
}

/// `--clear-all`: delete the database and attachments dir; prefs and exports stay.
fn run_clear_all(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let att = home()?.join(ATTACH_REL_PATH);
    let targets = maint::clear_all_targets(&db, &att);
    let (n, bytes) = maint::dir_usage(&att);
    if targets.is_empty() {
        eprintln!("nothing to clear ({} and {} do not exist)", db.display(), att.display());
        return Ok(());
    }
    eprintln!("clear-all will delete:");
    for t in &targets {
        if t.is_dir() { eprintln!("  {}/ ({n} files, {bytes} bytes)", t.display()); } else { eprintln!("  {}", t.display()); }
    }
    eprintln!("kept: {} and {}", home()?.join(PREFS_REL_PATH).display(), home()?.join(EXPORTS_REL_PATH).display());
    if args.dry_run {
        eprintln!("dry run: nothing changed");
        return Ok(());
    }
    if !maint::confirm("Delete all scraped data?", true, args.yes)? {
        eprintln!("aborted: nothing changed");
        return Ok(());
    }
    let gone = maint::clear_all(&db, &att)?;
    eprintln!("cleared: {} paths removed", gone.len());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    use clap::Parser;
    let args = Args::parse();
    if let Some(what) = args.generate {
        return generate(what, &mut io::stdout().lock());
    }
    let level = if args.quiet {
        LogLevel::Quiet
    } else if args.debug || std::env::var("ACP_DEBUG").is_ok_and(|v| v == "1") {
        LogLevel::Debug
    } else if args.verbose {
        LogLevel::Verbose
    } else {
        LogLevel::Normal
    };
    set_log_level(level);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let clean = |n: String| n.replace(['/', '\\'], "_");
    if let Some(list) = &args.remove_nick {
        return run_remove_nick(&args, &list.0);
    }
    if args.clear_all {
        return run_clear_all(&args);
    }
    if args.list_nicks {
        let infos = list_nicks_db(&home()?.join(DB_REL_PATH))?;
        let argv: Vec<String> = std::env::args().collect();
        let want_json = argv.windows(2).any(|w| w[0] == "--format" && w[1] == "json")
            || argv.iter().any(|a| a == "--format=json");
        if want_json {
            let v: Vec<_> = infos.iter().map(|i| json!({
                "nick": i.nick, "display_name": i.display_name, "post_count": i.post_count,
                "latest": i.latest, "profile_scraped": i.profile_scraped, "scraped_at": i.scraped_at,
            })).collect();
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else {
            print_nicks_table(&infos);
        }
        return Ok(());
    }
    if let Some(file) = &args.import {
        let (posts, atts, profiles, hits) = export::import(file, args.compression(), &home()?.join(ATTACH_REL_PATH), &home()?.join(EXPORTS_REL_PATH))?;
        let n = save(&posts)?;
        export::save_attachments(&home()?.join(DB_REL_PATH), &atts)?;
        for p in &profiles { save_profile(&home()?.join(DB_REL_PATH), p)?; }
        save_mentions(&home()?.join(DB_REL_PATH), &hits)?;
        info!("import: {} posts read, {n} new; {} attachments; {} profiles; {} mentions",
            posts.len(), atts.len(), profiles.len(), hits.len());
        return Ok(());
    }
    // Offline DB export: --export FILE, or --search (to stdout/--output).
    if args.export.is_some() || args.search.is_some() {
        let mut a = args.clone();
        if let Some(f) = &args.export { a.output = Some(f.clone()); }
        let q = args.search.as_deref();
        let nicks = args.nick_list()?;
        let (posts, atts, n) = export::search_db(&home()?.join(DB_REL_PATH), q, &nicks)?;
        info!("db export: {n} matches, {} posts total, {} attachments", posts.len(), atts.len());
        let name = clean(args.name.clone().unwrap_or_else(|| match q {
            Some(q) => format!("search-{}-{stamp}", export::slug(q)),
            None if !nicks.is_empty() => nicks_name(&nicks, &stamp),
            None => format!("db-{stamp}"),
        }));
        let who = nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
        let label = match (q, who.is_empty()) {
            (Some(q), true) => format!("search: {q}"),
            (Some(q), false) => format!("search: {q} ({who})"),
            (None, false) => who,
            (None, true) => "all posts".into(),
        };
        let (profiles, hits) = load_profiles_mentions(&home()?.join(DB_REL_PATH), &nicks)?;
        return export::export(&a, &name, &label, &posts, &atts, &profiles, &hits, &home()?.join(EXPORTS_REL_PATH));
    }
    let jar = Arc::new(Jar::default());
    let http = client(jar.clone())?;
    login(&http, &jar).await?;
    let mut nicks = args.nick_list()?;
    if nicks.is_empty() {
        nicks = parse_nicks(&prompt("Nick(s) to scrape (comma-separated): ")?).map_err(|e| anyhow!(e))?.0;
    }
    let db = home()?.join(DB_REL_PATH);
    let att_dir = home()?.join(ATTACH_REL_PATH);
    let known = db_known_ids(&db)?;
    let reply_counts = db_reply_counts(&db)?;
    let known_infos = list_nicks_db(&db)?;
    info!("== mode: {} (site crawl); profiles always use Latest", args.feed_mode());
    info!("== targets");
    for nick in &nicks {
        if let Some(i) = known_infos.iter().find(|i| i.nick.eq_ignore_ascii_case(nick)) {
            let dn = i.display_name.as_deref().unwrap_or("-");
            info!("  @{nick}: known ({dn}, {} posts{})",
                i.post_count,
                if i.profile_scraped { ", profile scraped" } else { "" });
        } else {
            info!("  @{nick}: new");
        }
    }
    let mut posts: Vec<Post> = Vec::new();
    let mut profiles: Vec<mentions::Profile> = Vec::new();
    let mut mention_hits: Vec<mentions::MentionHit> = Vec::new();
    let mut targets: Vec<mentions::MentionTarget> = Vec::new();
    let mut seen = HashSet::new();
    let mut summary = Vec::new();
    let mut ok_nicks = Vec::new();
    for nick in &nicks {
        info!("== scraping @{nick}");
        match scrape(&http, &jar, nick, &known, &reply_counts, args.full).await {
            Ok((ps, html, st)) => {
                info!("  @{nick}: feed {} items ({} from posts & replies), ~{} new, {} skipped known, threads fetched {} / skipped {}",
                    st.feed_items, st.replies_tab_items, st.new_posts, st.skipped_known, st.threads_fetched, st.threads_skipped);
                let mut profile = mentions::Profile {
                    nick: nick.clone(),
                    scraped_at: chrono::Local::now().to_rfc3339(),
                    ..Default::default()
                };
                mentions::enrich_from_html(&mut profile, &html);
                if let Ok(feed) = fetch_feed(&http, &jar, &PROFILE_URL.replace("{nick}", nick), Some(10), None, true).await {
                    mentions::enrich_from_feed(&mut profile, &feed.items, nick);
                }
                dbg_log("profile", &format!("@{nick} show-info unused event={PROFILE_SHOW_INFO_EVENT} fields={profile:?}"));
                if !args.no_attachments {
                    if let Some(u) = &profile.avatar_url
                        && let Ok((path, _)) = export::fetch_one_pub(&http, u, &att_dir).await {
                            profile.avatar_path = Some(path.to_string_lossy().into());
                        }
                    if let Some(u) = &profile.banner_url
                        && let Ok((path, _)) = export::fetch_one_pub(&http, u, &att_dir).await {
                            profile.banner_path = Some(path.to_string_lossy().into());
                        }
                }
                profile.raw_json = serde_json::to_string(&profile).unwrap_or_default();
                save_profile(&db, &profile)?;
                targets.push(mentions::MentionTarget {
                    nick: nick.clone(),
                    user_id: profile.user_id.clone(),
                    display_name: profile.display_name.clone(),
                });
                profiles.push(profile);

                let roots = ps.iter().filter(|p| p.parent_post_id.is_empty()).count();
                let total = ps.len();
                let new: Vec<Post> = ps.into_iter().filter(|p| seen.insert(p.post_id.clone())).collect();
                let inserted = save(&new)?;
                summary.push(format!(
                    "@{nick}: {total} items ({roots} root posts, {} replies), {inserted} new DB rows",
                    total - roots
                ));
                posts.extend(new);
                ok_nicks.push(nick.clone());
            }
            Err(e) => {
                err!("warning: @{nick} failed: {e:#}");
                summary.push(format!("@{nick}: FAILED ({e})"));
            }
        }
    }
    if args.want_mentions() && !ok_nicks.is_empty() {
        info!("== mentions crawl (Latest, limit {})", args.crawl_limit);
        match crawl_mentions(&http, &jar, &targets, args.crawl_limit, &mut seen, &known, args.full, args.feed_mode()).await {
            Ok((extra, hits)) => {
                let inserted = save(&extra)?;
                save_mentions(&db, &hits)?;
                info!("mentions: {} posts, {} hits, {inserted} new DB rows", extra.len(), hits.len());
                posts.extend(extra);
                mention_hits.extend(hits);
            }
            Err(e) => err!("warning: mentions crawl failed: {e:#}"),
        }
    }
    info!("== summary");
    for l in &summary { info!("  {l}"); }
    if ok_nicks.is_empty() { bail!("all nicks failed"); }
    let atts = if args.no_attachments { Vec::new() } else {
        let for_dl: Vec<Post> = if args.full {
            posts.clone()
        } else {
            // Only download media for posts we didn't already have.
            posts.iter().filter(|p| !known.contains(&p.post_id)).cloned().collect()
        };
        info!("attachments: downloading for {} posts ({} skipped as known)",
            for_dl.len(), posts.len().saturating_sub(for_dl.len()));
        let a = export::download_all(&http, &for_dl, &att_dir).await;
        export::save_attachments(&db, &a)?;
        a
    };
    let name = clean(args.name.clone().unwrap_or_else(|| nicks_name(&ok_nicks, &stamp)));
    let label = ok_nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
    export::export(&args, &name, &label, &posts, &atts, &profiles, &mention_hits, &home()?.join(EXPORTS_REL_PATH))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn nicks_parse() {
        assert_eq!(parse_nicks("@haz,@m3talsmith").unwrap().0, ["haz", "m3talsmith"]);
        assert_eq!(parse_nicks(" @a , b,,@A, @b ,c").unwrap().0, ["a", "b", "c"]);
        assert_eq!(parse_nicks("solo").unwrap().0, ["solo"]);
        assert!(parse_nicks(" , @ ,").is_err());
        assert!(parse_nicks("a b").is_err());
    }

    #[test]
    fn nicks_cli() {
        let a = Args::try_parse_from(["x", "--nicks", "@haz, @m3talsmith,@haz"]).unwrap();
        assert_eq!(a.nick_list().unwrap(), ["haz", "m3talsmith"]);
        let a = Args::try_parse_from(["x", "--nick", "@solo"]).unwrap();
        assert_eq!(a.nick_list().unwrap(), ["solo"]);
        assert!(Args::try_parse_from(["x", "--nick", "a", "--nicks", "b"]).is_err());
        assert!(Args::try_parse_from(["x", "--import", "f", "--nicks", "b"]).is_err());
        assert!(Args::try_parse_from(["x", "--nicks", ","]).is_err());
        assert!(Args::try_parse_from(["x", "--export", "f", "--nicks", "a,b"]).is_ok());
    }

    #[test]
    fn default_names() {
        assert_eq!(nicks_name(&["haz".into(), "m3talsmith".into()], "T"), "haz+m3talsmith-T");
        let many: Vec<String> = (0..5).map(|i| format!("n{i}")).collect();
        assert_eq!(nicks_name(&many, "T"), "multi-T");
    }

    #[test]
    fn incremental_page_stop() {
        let known: HashSet<String> = ["a", "b"].into_iter().map(str::to_string).collect();
        let page = vec![json!({"id":"a"}), json!({"id":"b"})];
        assert!(page_all_known(&page, &known));
        let page2 = vec![json!({"id":"a"}), json!({"id":"c"})];
        assert!(!page_all_known(&page2, &known));
        assert!(!page_all_known(&[], &known));
    }

    #[test]
    fn list_nicks_flag() {
        assert!(Args::try_parse_from(["x", "--list-nicks"]).is_ok());
        assert!(Args::try_parse_from(["x", "--full"]).is_ok());
    }

    #[test]
    fn quiet_verbose_conflict() {
        assert!(Args::try_parse_from(["x", "-q", "-v"]).is_err());
        assert!(Args::try_parse_from(["x", "--quiet", "--debug"]).is_err());
        assert!(Args::try_parse_from(["x", "-v", "--debug"]).is_ok());
        assert!(Args::try_parse_from(["x", "-q"]).is_ok());
    }

    const PROFILE: &str = "https://acp.us/rs/@alice";

    #[test]
    fn replies_tab_live_patch_link() {
        let h = r#"<nav><a href="/rs/@alice" data-phx-link="patch">Posts</a>
            <a href="/rs/@alice/replies" data-phx-link="patch" data-phx-link-state="push">Posts &amp; replies</a></nav>"#;
        assert_eq!(find_replies_tab(h, PROFILE), Some(RepliesTab::Link("https://acp.us/rs/@alice/replies".into())));
    }

    #[test]
    fn replies_tab_query_param_link() {
        let h = r#"<a href="?tab=replies">Posts and Replies</a>"#;
        assert_eq!(find_replies_tab(h, PROFILE), Some(RepliesTab::Link("https://acp.us/rs/@alice?tab=replies".into())));
    }

    #[test]
    fn replies_tab_plain_phx_click() {
        let h = r#"<button phx-click="Posts">Posts</button>
            <button phx-click="switch-tab" phx-value-tab="posts_and_replies" phx-target="3"><span>Posts</span> &amp; <span>replies</span></button>"#;
        let mut v = serde_json::Map::new();
        v.insert("tab".into(), json!("posts_and_replies"));
        assert_eq!(find_replies_tab(h, PROFILE), Some(RepliesTab::Event { event: "switch-tab".into(), value: v, cid: Some(3) }));
    }

    #[test]
    fn replies_tab_js_push_and_patch() {
        let push = r#"<button phx-click="[[&quot;push&quot;,{&quot;event&quot;:&quot;set-filter&quot;,&quot;value&quot;:{&quot;filter&quot;:&quot;replies&quot;}}]]">Posts &amp; replies</button>"#;
        match find_replies_tab(push, PROFILE) {
            Some(RepliesTab::Event { event, value, cid }) => {
                assert_eq!(event, "set-filter");
                assert_eq!(value["filter"], "replies");
                assert_eq!(cid, None);
            }
            other => panic!("{other:?}"),
        }
        let patch = r#"<a phx-click="[[&quot;patch&quot;,{&quot;href&quot;:&quot;/rs/@alice/with_replies&quot;}]]">Posts &amp; replies</a>"#;
        assert_eq!(find_replies_tab(patch, PROFILE), Some(RepliesTab::Link("https://acp.us/rs/@alice/with_replies".into())));
    }

    #[test]
    fn replies_tab_absent_or_only_posts() {
        assert_eq!(find_replies_tab(r#"<a href="/rs/@alice">Posts</a><a href="/rs/@alice/top">Top</a>"#, PROFILE), None);
        assert_eq!(find_replies_tab("<p>Posts &amp; replies</p>", PROFILE), None); // no link/click
    }

    #[test]
    fn merge_dedupes_by_id() {
        let mut items = vec![json!({"id": "a"}), json!({"id": "b"})];
        let n = merge_feed_items(&mut items, vec![json!({"id": "b"}), json!({"id": "c", "thread_id": "x"}), json!({"id": ""}), json!({"id": "c"})]);
        assert_eq!(n, 1);
        let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[test]
    fn reply_from_replies_tab_gets_thread_parent() {
        let p = to_post(&json!({"id": "r1", "thread_id": "t9", "inserted_at": "2026-10-15T00:00:00Z",
            "author": {"username": "alice"}, "body": {}}), "t9", "alice");
        assert_eq!((p.post_id.as_str(), p.parent_post_id.as_str()), ("r1", "t9"));
    }

    #[test]
    fn generators_render() {
        for g in [Generate::Man, Generate::Bash, Generate::Zsh, Generate::Fish] {
            let mut v = Vec::new();
            generate(g, &mut v).unwrap();
            let t = String::from_utf8(v).unwrap().replace("\\-", "-");
            assert!(t.contains("remove-nick"), "{g:?}");
        }
    }

    #[test]
    fn maintenance_flags() {
        let a = Args::try_parse_from(["x", "--remove-nick", "@a, b,@A", "--dry-run", "-y", "--keep-replies"]).unwrap();
        assert_eq!(a.remove_nick.unwrap().0, vec!["a", "b"]);
        assert!(a.dry_run && a.yes && a.keep_replies);
        assert!(Args::try_parse_from(["x", "--clear-all", "--yes"]).unwrap().clear_all);
        for bad in [
            &["x", "--remove-nick", "a", "--clear-all"][..],
            &["x", "--remove-nick", "a", "--nick", "b"],
            &["x", "--remove-nick", "a", "--export", "f.json"],
            &["x", "--clear-all", "--import", "f.json"],
            &["x", "--clear-all", "--search", "q"],
            &["x", "--clear-all", "--keep-replies"],
            &["x", "--dry-run"],
            &["x", "--yes"],
        ] {
            assert!(Args::try_parse_from(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn feed_mode_flags() {
        let a = Args::try_parse_from(["x"]).unwrap();
        assert_eq!(a.feed_mode(), FeedMode::Latest);
        let a = Args::try_parse_from(["x", "--latest"]).unwrap();
        assert_eq!(a.feed_mode(), FeedMode::Latest);
        let a = Args::try_parse_from(["x", "--trending"]).unwrap();
        assert_eq!(a.feed_mode(), FeedMode::Trending);
        assert!(Args::try_parse_from(["x", "--latest", "--trending"]).is_err());
        assert_eq!(FeedMode::Latest.site_feed_url(), FEED_LATEST_URL);
        assert_eq!(FeedMode::Trending.site_feed_url(), FEED_TRENDING_URL);
        assert!(FeedMode::Latest.allow_incremental_stop());
        assert!(!FeedMode::Trending.allow_incremental_stop());
    }
}
