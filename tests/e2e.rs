//! Gherkin end-to-end tests: the real binary against a mock acp.us
//! (`tests/support/mock.rs`), with a temporary `HOME`. Never touches the real
//! site: `ACP_BASE_URL` points every request at the local mock.
//!
//! Run with `cargo test --test e2e`. Feature files live in `tests/features/`.

mod support;

use cucumber::{World, given, then, when};
use rusqlite::Connection;
use std::path::PathBuf;
use support::mock::{CODE, Mock};
use tokio::io::AsyncWriteExt;
use acp_rs_scraper::tui::commands::{CommandId, Menu};

/// Output of one run of the binary.
#[derive(Debug, Default)]
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Per-scenario state: the mock site, a throwaway HOME, the last run.
#[derive(Debug, World)]
#[world(init = Self::new)]
struct E2e {
    mock: Option<Mock>,
    home: PathBuf,
    last: Run,
    code: String,
    counters_before: (usize, usize),
    menu: Option<Menu>,
    /// Extra environment for the runs (`KEY` → value).
    env: Vec<(String, String)>,
    /// Theme downloads go here instead of the mock (e.g. a closed port: offline).
    theme_url: Option<String>,
}

impl E2e {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!("acp-e2e-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(home.join(".acp")).unwrap();
        std::fs::write(home.join(".acp/preferences.toml"), "email = \"alice@example.test\"\n").unwrap();
        E2e { mock: None, home, last: Run::default(), code: CODE.into(), counters_before: (0, 0), menu: None, env: Vec::new(), theme_url: None }
    }
    fn mock(&self) -> &Mock { self.mock.as_ref().expect("mock site not started") }
    fn db(&self) -> PathBuf { self.home.join(".acp/ai/data/rs.database") }
    fn att(&self) -> PathBuf { self.home.join(".acp/ai/data/attachments") }
    fn q(&self, sql: &str) -> i64 {
        Connection::open(self.db()).unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    async fn run(&mut self, args: &str) {
        let argv: Vec<String> = args.split_whitespace().map(String::from).collect();
        self.run_argv(&argv).await;
    }
    /// Run the binary with these arguments (no `argv[0]`); `$HOME` is expanded.
    async fn run_argv(&mut self, args: &[String]) {
        let base = self.mock.as_ref().map(|m| m.base.clone()).unwrap_or_else(|| "http://127.0.0.1:9".into());
        let args: Vec<String> = args.iter().map(|a| a.replace("$HOME", &self.home.to_string_lossy()).replace("$SITE", &base)).collect();
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_acp-rs-scraper"))
            .args(&args).env("HOME", &self.home).env("ACP_THEME_URL", self.theme_url.clone().unwrap_or_else(|| format!("{base}/themes/")))
            .env("ACP_BASE_URL", base).env_remove("ACP_DEBUG").env_remove("ACP_THEME_DOWNLOAD").envs(self.env.iter().cloned())
            .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
            .spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(format!("{}\n", self.code).as_bytes()).await.unwrap();
        drop(stdin);
        let out = child.wait_with_output().await.unwrap();
        self.last = Run { code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into(), stderr: String::from_utf8_lossy(&out.stderr).into() };
    }
}

impl Drop for E2e {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.home); }
}

#[given("a mock Red Star site")]
async fn site(w: &mut E2e) { w.mock = Some(Mock::start().await); }

#[given("the site's \"Posts and replies\" URL is broken")]
async fn broken_replies(w: &mut E2e) { w.mock().inner.lock().unwrap().replies_url_broken = true; }

#[given(expr = "I will enter the code {string}")]
async fn will_enter(w: &mut E2e, code: String) { w.code = code; }

#[given(expr = "I have run {string}")]
#[when(expr = "I run {string}")]
async fn run(w: &mut E2e, args: String) {
    if let Some(m) = &w.mock { let i = m.inner.lock().unwrap(); w.counters_before = (i.page_next, i.thread_gets); }
    w.run(&args).await;
}

impl E2e {
    fn menu(&mut self) -> &mut Menu {
        use clap::Parser;
        self.menu.get_or_insert_with(|| Menu::new(&acp_rs_scraper::cli::Args::parse_from(["acp-rs-scraper"])))
    }
}

#[given(expr = "the TUI Scrape menu has {string} off")]
async fn scrape_opt_off(w: &mut E2e, label: String) {
    let key = w.menu().items.iter().find(|i| i.id == CommandId::Scrape).unwrap()
        .opts.iter().find(|o| o.label == label).unwrap().key;
    w.menu().set_check(CommandId::Scrape, key, false);
}

#[when(expr = "I start scanning from the TUI with the nicks {string} selected")]
async fn tui_scan(w: &mut E2e, nicks: String) {
    let sel: Vec<String> = nicks.split(',').map(String::from).collect();
    let argv = w.menu().scrape_argv(&sel).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[when(expr = "I export the TUI selection {string} to {string} as {string}")]
async fn tui_export(w: &mut E2e, nicks: String, path: String, format: String) {
    let sel: Vec<String> = nicks.split(',').map(String::from).collect();
    w.menu().set_text(CommandId::Export, "path", &path);
    w.menu().set_choice(CommandId::Export, "format", &format);
    let argv = w.menu().export_argv(&sel, None).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[when(expr = "I export the TUI basket {string} to {string}")]
async fn tui_basket(w: &mut E2e, ids: String, path: String) {
    let basket: Vec<String> = ids.split(',').map(String::from).collect();
    w.menu().set_text(CommandId::Export, "path", &path);
    let argv = w.menu().export_argv_with(&[], None, &basket, Some("Basket")).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[when(expr = "I remove the TUI selection {string} from the Maintain menu")]
async fn tui_remove(w: &mut E2e, nicks: String) {
    let sel: Vec<String> = nicks.split(',').map(String::from).collect();
    let argv = w.menu().remove_argv(&sel).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[when(expr = "I bookmark {string} from the TUI")]
async fn tui_bookmark(w: &mut E2e, id: String) {
    let argv = w.menu().bookmark_argv(&id, true).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[when(expr = "I sync from the TUI with the nicks {string} selected")]
async fn tui_sync(w: &mut E2e, nicks: String) {
    let sel: Vec<String> = nicks.split(',').map(String::from).collect();
    let argv = w.menu().sync_argv(&sel).unwrap();
    w.run_argv(&argv[1..]).await;
}

#[then(expr = "stdout does not contain {string}")]
async fn stdout_lacks(w: &mut E2e, s: String) { assert!(!w.last.stdout.contains(&s), "stdout has {s:?}:\n{}", w.last.stdout); }

#[then("it succeeds")]
async fn succeeds(w: &mut E2e) { assert_eq!(w.last.code, 0, "stderr:\n{}", w.last.stderr); }

#[then("it fails")]
async fn fails(w: &mut E2e) { assert_ne!(w.last.code, 0, "unexpected success:\n{}", w.last.stderr); }

#[then(expr = "stderr contains {string}")]
async fn stderr_has(w: &mut E2e, s: String) { assert!(w.last.stderr.contains(&s), "stderr lacks {s:?}:\n{}", w.last.stderr); }

#[then(expr = "stdout contains {string}")]
async fn stdout_has(w: &mut E2e, s: String) { assert!(w.last.stdout.contains(&s), "stdout lacks {s:?}:\n{}", w.last.stdout); }

#[then(expr = "the site has bookmarks {string}")]
async fn site_marks(w: &mut E2e, list: String) {
    let mut got = w.mock().inner.lock().unwrap().site_bookmarks.clone();
    got.sort();
    let mut want: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    want.sort();
    assert_eq!(got, want);
}

#[then(expr = "my bookmarks are {string}")]
async fn local_marks(w: &mut E2e, list: String) {
    let c = Connection::open(w.db()).unwrap();
    let mut st = c.prepare("SELECT post_id FROM bookmarks WHERE deleted = 0 ORDER BY post_id").unwrap();
    let got: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    let mut want: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    want.sort();
    assert_eq!(got, want);
}

#[then(expr = "the site sent a login code to {string}")]
async fn code_sent(w: &mut E2e, e: String) { assert!(w.mock().inner.lock().unwrap().login_emails.contains(&e)); }

#[then(expr = "the database has {int} posts")]
async fn n_posts(w: &mut E2e, n: i64) { assert_eq!(w.q("SELECT count(*) FROM posts"), n); }

#[then(expr = "the database has {int} posts by {string}")]
async fn n_posts_by(w: &mut E2e, n: i64, nick: String) {
    assert_eq!(w.q(&format!("SELECT count(*) FROM posts WHERE nick='{nick}'")), n);
}

#[then(expr = "post {string} has parent {string}")]
async fn parent(w: &mut E2e, id: String, p: String) {
    let got: String = Connection::open(w.db()).unwrap()
        .query_row("SELECT parent_post_id FROM posts WHERE post_id=?1", [&id], |r| r.get(0)).unwrap();
    assert_eq!(got, p);
}

#[then(expr = "post {string} is stored once")]
async fn once(w: &mut E2e, id: String) { assert_eq!(w.q(&format!("SELECT count(*) FROM posts WHERE post_id='{id}'")), 1); }

#[then(expr = "post {string} is not stored")]
async fn not_stored(w: &mut E2e, id: String) { assert_eq!(w.q(&format!("SELECT count(*) FROM posts WHERE post_id='{id}'")), 0); }

#[then(expr = "{string} is recorded as mentioning {string}")]
async fn mention(w: &mut E2e, id: String, nick: String) {
    assert!(w.q(&format!("SELECT count(*) FROM mentions WHERE post_id='{id}' AND mentioned_nick='{nick}'")) > 0);
}

#[then(expr = "{int} attachment files are on disk")]
async fn files(w: &mut E2e, n: usize) {
    let c = std::fs::read_dir(w.att()).map(|d| d.count()).unwrap_or(0);
    assert_eq!(c, n);
}

#[then(expr = "{int} media files are on disk")]
async fn media_files(w: &mut E2e, n: usize) {
    let c = std::fs::read_dir(w.att()).map(|d| d.flatten().filter(|e| e.path().is_file()).count()).unwrap_or(0);
    assert_eq!(c, n);
}

#[then(expr = "the attachments table has {int} rows")]
async fn att_rows(w: &mut E2e, n: i64) { assert_eq!(w.q("SELECT count(*) FROM attachments"), n); }

#[then(expr = "the site served {int} feed pages via page-next")]
async fn pages(w: &mut E2e, n: usize) { assert_eq!(w.mock().inner.lock().unwrap().page_next - w.counters_before.0, n); }

#[then(expr = "the site served at most {int} feed pages via page-next")]
async fn pages_max(w: &mut E2e, n: usize) { assert!(w.mock().inner.lock().unwrap().page_next - w.counters_before.0 <= n); }

#[then(expr = "the site served at least {int} feed pages via page-next")]
async fn pages_min(w: &mut E2e, n: usize) { assert!(w.mock().inner.lock().unwrap().page_next - w.counters_before.0 >= n); }

#[then(expr = "the site served {int} thread pages")]
async fn threads(w: &mut E2e, n: usize) { assert_eq!(w.mock().inner.lock().unwrap().thread_gets - w.counters_before.1, n); }

#[then("the profile filter form was used")]
async fn form_used(w: &mut E2e) { assert!(w.mock().inner.lock().unwrap().configure_feed > 0); }

#[then("the \"Posts and replies\" URL was requested")]
async fn url_used(w: &mut E2e) { assert!(w.mock().inner.lock().unwrap().replies_url_gets > 0); }

#[then("the database file is gone")]
async fn db_gone(w: &mut E2e) { assert!(!w.db().exists()); }

#[then("the attachments directory is gone")]
async fn att_gone(w: &mut E2e) { assert!(!w.att().exists()); }

#[then("my preferences are kept")]
async fn prefs_kept(w: &mut E2e) { assert!(w.home.join(".acp/preferences.toml").exists()); }

#[then(expr = "the file {string} exists")]
async fn file_exists(w: &mut E2e, f: String) { assert!(w.home.join(f.trim_start_matches("$HOME/")).exists()); }

#[then(expr = "the file {string} does not exist")]
async fn file_missing(w: &mut E2e, f: String) { assert!(!w.home.join(f.trim_start_matches("$HOME/")).exists()); }

#[then(expr = "the file {string} does not contain {string}")]
async fn file_lacks(w: &mut E2e, f: String, text: String) {
    let p = f.replace("$HOME", &w.home.to_string_lossy());
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
    assert!(!s.contains(&text), "{p} contains {text:?}");
}

#[then(expr = "the file {string} contains {string}")]
async fn file_contains(w: &mut E2e, f: String, s: String) {
    let t = std::fs::read_to_string(w.home.join(f.trim_start_matches("$HOME/"))).unwrap();
    assert!(t.contains(&s), "{f} lacks {s}");
}

#[then(expr = "post {string} has content {string}")]
async fn content(w: &mut E2e, id: String, c: String) {
    let got: String = Connection::open(w.db()).unwrap()
        .query_row("SELECT content FROM posts WHERE post_id=?1", [&id], |r| r.get(0)).unwrap();
    assert_eq!(got.trim(), c);
}

#[then(expr = "post {string} has {int} paragraphs ending {string}")]
async fn paragraphs(w: &mut E2e, id: String, n: usize, end: String) {
    let got: String = Connection::open(w.db()).unwrap()
        .query_row("SELECT content FROM posts WHERE post_id=?1", [&id], |r| r.get(0)).unwrap();
    assert_eq!(got.split("\n\n").count(), n, "{got:?}");
    assert!(got.ends_with(&end), "{got:?}");
}

#[then(expr = "post {string} has body state {string}")]
async fn body_state(w: &mut E2e, id: String, st: String) {
    let got: String = Connection::open(w.db()).unwrap()
        .query_row("SELECT body_state FROM posts WHERE post_id=?1", [&id], |r| r.get(0)).unwrap();
    assert_eq!(got, st);
}

#[given(expr = "post {string} was stored truncated as {string}")]
async fn stored_truncated(w: &mut E2e, id: String, st: String) {
    Connection::open(w.db()).unwrap().execute("UPDATE posts SET content='short…', body_state=?2 WHERE post_id=?1", [&id, &st]).unwrap();
}

#[given(expr = "post {string} was stored blank")]
async fn blank(w: &mut E2e, id: String) {
    Connection::open(w.db()).unwrap().execute("UPDATE posts SET content='', title='' WHERE post_id=?1", [&id]).unwrap();
}

#[then(expr = "the file {string} shows the profile image {string}")]
async fn shows_avatar(w: &mut E2e, f: String, src: String) {
    let t = std::fs::read_to_string(w.home.join(f.trim_start_matches("$HOME/"))).unwrap();
    assert!(t.contains(&format!("<img class=\"avatar\" src=\"{src}\"")), "{f} has no avatar {src}");
}

#[then(expr = "post {string} replies directly to {string}")]
async fn direct_parent(w: &mut E2e, id: String, p: String) {
    let got: String = Connection::open(w.db()).unwrap()
        .query_row("SELECT in_reply_to_id FROM posts WHERE post_id=?1", [&id], |r| r.get(0)).unwrap();
    assert_eq!(got, p);
}

/// The `<article id="post-{id}">` block of `id` up to its first nested article or end.
fn article(html: &str, id: &str) -> String {
    let start = html.find(&format!("id=\"post-{id}\"")).unwrap_or_else(|| panic!("no #post-{id}"));
    let rest = &html[start..];
    let end = rest[1..].find("<article").map(|i| i + 1).unwrap_or(rest.len());
    rest[..end].to_string()
}

#[then(expr = "the file {string} links {string} to its parent {string}")]
async fn links_parent(w: &mut E2e, f: String, id: String, href: String) {
    let t = std::fs::read_to_string(w.home.join(f.trim_start_matches("$HOME/"))).unwrap();
    let a = article(&t, &id);
    assert!(a.contains(&format!("class=\"parent\" href=\"{href}\"")), "{id} lacks parent link {href}: {a}");
    if let Some(target) = href.strip_prefix('#') { assert!(t.contains(&format!("id=\"{target}\"")), "anchor {target} missing"); }
}

#[then(expr = "the file {string} links {string} to its thread {string}")]
async fn links_thread(w: &mut E2e, f: String, id: String, path: String) {
    let t = std::fs::read_to_string(w.home.join(f.trim_start_matches("$HOME/"))).unwrap();
    let a = article(&t, &id);
    let base = &w.mock().base;
    assert!(a.contains(&format!("class=\"thread\" href=\"{base}{path}\"")), "{id} lacks thread link {path}: {a}");
}

#[tokio::main]
async fn main() {
    E2e::cucumber().max_concurrent_scenarios(4).fail_on_skipped().run_and_exit("tests/features").await;
}

#[given(expr = "a local file {string} containing {string}")]
async fn local_file(w: &mut E2e, f: String, text: String) {
    let p = std::path::PathBuf::from(f.replace("$HOME", &w.home.to_string_lossy()));
    if let Some(d) = p.parent() { std::fs::create_dir_all(d).unwrap(); }
    std::fs::write(p, text.replace("\\n", "\n")).unwrap();
}

#[then(expr = "research project {string} has {int} items")]
async fn research_count(w: &mut E2e, name: String, n: i64) {
    let got = w.q(&format!("SELECT count(*) FROM research_items i JOIN research_projects p ON p.id = i.project_id WHERE p.name = '{name}' COLLATE NOCASE"));
    assert_eq!(got, n);
}

#[then(expr = "research project {string} has a {string} item {string}")]
async fn research_item(w: &mut E2e, name: String, kind: String, r#ref: String) {
    let r#ref = r#ref.replace("$HOME", &w.home.to_string_lossy()).replace("$SITE", &w.mock().base);
    let got = w.q(&format!("SELECT count(*) FROM research_items i JOIN research_projects p ON p.id = i.project_id
        WHERE p.name = '{name}' COLLATE NOCASE AND i.kind = '{kind}' AND i.ref = '{}'", r#ref.replace('\'', "''")));
    assert_eq!(got, 1, "{kind} {ref}");
}

#[then("the web page was fetched without the site's cookies")]
async fn web_no_cookies(w: &mut E2e) {
    let c = w.mock().inner.lock().unwrap().web_cookies.clone();
    assert!(c.iter().all(|c| !c.contains("_acp_key")), "{c:?}");
}

#[when(expr = "I add {string} to research project {string} from the TUI")]
async fn tui_research_add(w: &mut E2e, id: String, name: String) {
    let argv = acp_rs_scraper::tui::commands::research_add_argv(&name, "", &acp_rs_scraper::tui::commands::ResearchSource::Posts(vec![id]), "").unwrap();
    w.run_argv(&argv[1..]).await;
}

#[given(expr = "the theme server answers {string}")]
async fn theme_server(w: &mut E2e, mode: String) {
    if mode == "offline" { w.theme_url = Some("http://127.0.0.1:9/themes/".into()); } else { w.mock().inner.lock().unwrap().theme_mode = mode; }
}

#[given(expr = "the environment has {string} set to {string}")]
async fn env_set(w: &mut E2e, k: String, v: String) { w.env.push((k, v)); }

#[then(expr = "the theme server was asked {int} times")]
async fn theme_gets(w: &mut E2e, n: usize) { assert_eq!(w.mock().inner.lock().unwrap().theme_gets, n); }

#[then("no cookies were sent for themes")]
async fn theme_no_cookies(w: &mut E2e) {
    let c = w.mock().inner.lock().unwrap().theme_cookies.clone();
    assert!(c.is_empty(), "{c:?}");
}
