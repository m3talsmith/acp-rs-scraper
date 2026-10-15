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
}

impl E2e {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!("acp-e2e-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(home.join(".acp")).unwrap();
        std::fs::write(home.join(".acp/preferences.toml"), "email = \"alice@example.test\"\n").unwrap();
        E2e { mock: None, home, last: Run::default(), code: CODE.into(), counters_before: (0, 0) }
    }
    fn mock(&self) -> &Mock { self.mock.as_ref().expect("mock site not started") }
    fn db(&self) -> PathBuf { self.home.join(".acp/ai/data/rs.database") }
    fn att(&self) -> PathBuf { self.home.join(".acp/ai/data/attachments") }
    fn q(&self, sql: &str) -> i64 {
        Connection::open(self.db()).unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    async fn run(&mut self, args: &str) {
        let base = self.mock.as_ref().map(|m| m.base.clone()).unwrap_or_else(|| "http://127.0.0.1:9".into());
        let args: Vec<String> = args.split_whitespace().map(|a| a.replace("$HOME", &self.home.to_string_lossy())).collect();
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_acp-rs-scraper"))
            .args(&args).env("HOME", &self.home).env("ACP_BASE_URL", base).env_remove("ACP_DEBUG")
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

#[then("it succeeds")]
async fn succeeds(w: &mut E2e) { assert_eq!(w.last.code, 0, "stderr:\n{}", w.last.stderr); }

#[then("it fails")]
async fn fails(w: &mut E2e) { assert_ne!(w.last.code, 0, "unexpected success:\n{}", w.last.stderr); }

#[then(expr = "stderr contains {string}")]
async fn stderr_has(w: &mut E2e, s: String) { assert!(w.last.stderr.contains(&s), "stderr lacks {s:?}:\n{}", w.last.stderr); }

#[then(expr = "stdout contains {string}")]
async fn stdout_has(w: &mut E2e, s: String) { assert!(w.last.stdout.contains(&s), "stdout lacks {s:?}:\n{}", w.last.stdout); }

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

#[tokio::main]
async fn main() {
    E2e::cucumber().max_concurrent_scenarios(4).fail_on_skipped().run_and_exit("tests/features").await;
}
