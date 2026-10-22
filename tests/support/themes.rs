//! A local file server for theme downloads and one research article.
//! It does not speak for a site: no login, no feeds, no sockets.

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::{Arc, Mutex};

/// One-time code kept so older steps still compile. This server never checks it.
pub const CODE: &str = "KWMRUV";

/// Counters the end-to-end steps still read. Site counters stay at zero.
#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct Inner {
    /// Cookie headers sent with `/web/...` requests.
    pub web_cookies: Vec<String>,
    /// Unused. Feed paging is not served here.
    pub page_next: usize,
    /// Unused. Thread pages are not served here.
    pub thread_gets: usize,
    /// Unused.
    pub configure_feed: usize,
    /// Unused.
    pub replies_url_gets: usize,
    /// Unused.
    pub replies_url_broken: bool,
    /// Unused. This server does not send a login code.
    pub login_emails: Vec<String>,
    /// Unused.
    pub site_bookmarks: Vec<String>,
    /// Unused.
    pub bookmark_toggles: usize,
    /// How `/themes/default.toml` answers: empty or `ok` reads the repo file. `big`, `bad`, `slow`, and `missing` are the refusal cases.
    pub theme_mode: String,
    /// `/themes/...` requests received.
    pub theme_gets: usize,
    /// Cookie headers sent with `/themes/...` requests. There must be none.
    pub theme_cookies: Vec<String>,
    /// Unused. Explore is not served here.
    pub explore_hang: bool,
    /// Unused.
    pub edit_validates: usize,
    /// Unused.
    pub edit_saves: usize,
    /// Unused.
    pub edit_validate_value: String,
    /// Unused.
    pub edit_save_value: String,
}

/// Shared server state.
#[derive(Clone, Default)]
pub struct Mock {
    /// Base URL, for example `http://127.0.0.1:12345`.
    pub base: String,
    /// Counters and switches.
    pub inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for Mock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mock({})", self.base)
    }
}

/// The repo's `themes/` directory. `theme_mode` changes how `default.toml` answers.
async fn theme_file(State(m): State<Mock>, headers: HeaderMap, Path(file): Path<String>) -> Response {
    let mode = {
        let mut i = m.inner.lock().unwrap();
        i.theme_gets += 1;
        if let Some(c) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
            i.theme_cookies.push(c.to_string());
        }
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

/// One article for research `--url` checks.
async fn web_article(State(m): State<Mock>, headers: HeaderMap) -> Response {
    if let Some(c) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        m.inner.lock().unwrap().web_cookies.push(c.to_string());
    }
    let html = r#"<!DOCTYPE html><html><head><title>The Great Railroad Strike</title></head><body>
<nav><a href="/">Home</a> menu text</nav>
<article><h1>The Great Railroad Strike of 1877</h1><p>Workers stopped the trains in <b>Martinsburg</b>.</p>
<ul><li>Baltimore</li><li>Pittsburgh</li></ul><blockquote>An injury to one is an injury to all.</blockquote></article>
<footer>footer text</footer><script>var tracking = 1;</script></body></html>"#;
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

impl Mock {
    /// Listen on an ephemeral localhost port.
    pub async fn start() -> Mock {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut m = Mock {
            base: format!("http://{}", listener.local_addr().unwrap()),
            ..Default::default()
        };
        m.inner = Arc::new(Mutex::new(Inner {
            site_bookmarks: vec!["C1".into()],
            ..Default::default()
        }));
        let app = Router::new()
            .route("/web/article", axum::routing::get(web_article))
            .route("/themes/{file}", axum::routing::get(theme_file))
            .with_state(m.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        m
    }
}
