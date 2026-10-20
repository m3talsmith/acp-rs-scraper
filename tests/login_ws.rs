//! Login against a local Phoenix-shaped websocket.
//!
//! This is its own binary so `ACP_BASE_URL` is set before [`base_url`] latches.
//! Lib tests share one process and may already have latched the real origin.

use std::sync::Mutex;
use std::time::Duration;

use acp_red_star::auth::{LoginSession, LoginStart};
use acp_red_star::http::client;
use acp_red_star::prelude::{Arc, Jar, Result, Value, json};
use axum::{
    Router,
    extract::{State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;

const CODE: &str = "KWMRUV";
const SESSION: &str = "_mock_session";

#[derive(Default)]
struct Ctl {
    joins: usize,
    verifies: usize,
    /// `"hb"` and `"verify"`, in order.
    events: Vec<&'static str>,
    close_next_verify: bool,
    /// Verify pushes the sign-in form as a `diff`, then closes, with no `phx_reply`.
    diff_then_close: bool,
    /// Blind OTP (no `user[token]=from-form`) is rejected.
    require_form_token: bool,
    /// A new socket is refused while another handler is still running.
    one_at_a_time: bool,
    /// Verify sends Close but stays in the read loop until the client drops the socket.
    hold_close: bool,
    open: usize,
    /// Handlers that connected while `one_at_a_time` was already on.
    gated_open: usize,
    rejected: usize,
    pongs: usize,
    ping_after_email: bool,
}

fn lv_page() -> String {
    r#"<!DOCTYPE html><html><head><meta name="csrf-token" content="mock-csrf"></head><body>
<div data-phx-main id="phx-mock" data-phx-session="mock-session" data-phx-static="mock-static"></div></body></html>"#.into()
}

fn logged_in(h: &HeaderMap) -> bool {
    h.get_all(header::COOKIE).iter().any(|c| c.to_str().unwrap_or("").contains(&format!("{SESSION}=ok")))
}

async fn site(headers: HeaderMap) -> Response {
    if !logged_in(&headers) {
        return (StatusCode::FOUND, [(header::LOCATION, "/auth")]).into_response();
    }
    ([(header::CONTENT_TYPE, "text/html")], "<html><body>feed</body></html>").into_response()
}

async fn auth_page() -> Response {
    ([(header::CONTENT_TYPE, "text/html")], lv_page()).into_response()
}

async fn sign_in(State(ctl): State<Arc<Mutex<Ctl>>>, body: String) -> Response {
    let require = ctl.lock().unwrap().require_form_token;
    let code_ok = body.split('&').any(|kv| kv == format!("user%5Bcode%5D={CODE}") || kv == format!("user[code]={CODE}"));
    let token_ok = !require || body.split('&').any(|kv| kv == "user%5Btoken%5D=from-form" || kv == "user[token]=from-form");
    if !(code_ok && token_ok) { return (StatusCode::FORBIDDEN, "bad code").into_response(); }
    (StatusCode::FOUND, [(header::SET_COOKIE, format!("{SESSION}=ok; Path=/; HttpOnly")), (header::LOCATION, "/rs".to_string())]).into_response()
}

fn phx_reply(msg: &Value, response: Value) -> Message {
    Message::Text(json!([msg[0], msg[1], msg[2], "phx_reply", {"status": "ok", "response": response}]).to_string().into())
}

async fn live(State(ctl): State<Arc<Mutex<Ctl>>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| lv_conn(ctl, socket))
}

async fn lv_conn(ctl: Arc<Mutex<Ctl>>, mut ws: WebSocket) {
    let gated = ctl.lock().unwrap().one_at_a_time;
    if gated {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        loop {
            if ctl.lock().unwrap().gated_open == 0 { break; }
            if tokio::time::Instant::now() >= deadline {
                ctl.lock().unwrap().rejected += 1;
                let _ = ws.send(Message::Close(None)).await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    {
        let mut c = ctl.lock().unwrap();
        c.open += 1;
        if c.one_at_a_time { c.gated_open += 1; }
    }
    let _slot = OpenSlot { ctl: ctl.clone(), gated: ctl.lock().unwrap().one_at_a_time };
    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Pong(_) => { ctl.lock().unwrap().pongs += 1; }
            Message::Ping(p) => { if ws.send(Message::Pong(p)).await.is_err() { break; } }
            Message::Close(_) => break,
            Message::Text(t) => {
                if handle_text(&ctl, &mut ws, t.as_str()).await { break; }
            }
            _ => {}
        }
    }
}

/// `true` means the handler should stop reading.
async fn handle_text(ctl: &Arc<Mutex<Ctl>>, ws: &mut WebSocket, text: &str) -> bool {
    let Ok(msg) = serde_json::from_str::<Value>(text) else { return false };
    let event = msg[3].as_str().unwrap_or("");
    if event == "heartbeat" {
        ctl.lock().unwrap().events.push("hb");
        return ws.send(phx_reply(&msg, json!({}))).await.is_err();
    }
    let kind = msg[4]["event"].as_str().unwrap_or("");
    let out = match event {
        "phx_join" => {
            ctl.lock().unwrap().joins += 1;
            let form = r#"<form phx-submit="send-login-email" phx-target="1"><input type="email" name="user[email]"></form>"#;
            phx_reply(&msg, json!({"rendered": {"s": [form]}}))
        }
        "event" if kind == "send-login-email" => {
            let ping = ctl.lock().unwrap().ping_after_email;
            let reply = phx_reply(&msg, json!({"diff": {"s": [r#"<form phx-submit="verify-code"><input type="text" name="user[code]"></form>"#]}}));
            if ws.send(reply).await.is_err() { return true; }
            if ping && ws.send(Message::Ping(vec![9, 9, 9].into())).await.is_err() { return true; }
            return false;
        }
        "event" if kind == "verify-code" => {
            let (close, diff_close, hold) = {
                let mut c = ctl.lock().unwrap();
                c.verifies += 1;
                c.events.push("verify");
                let close = c.close_next_verify;
                if close { c.close_next_verify = false; }
                let diff_close = c.diff_then_close;
                let hold = c.hold_close;
                (close, diff_close, hold)
            };
            if diff_close {
                let form = r#"<form phx-trigger-action action="/auth/user/otp/sign_in" method="post"><input type="hidden" name="user[email]" value=""><input type="hidden" name="user[code]" value=""><input type="hidden" name="user[token]" value="from-form"></form>"#;
                let diff = json!([Value::Null, Value::Null, msg[2], "diff", {"s": [form]}]);
                let _ = ws.send(Message::Text(diff.to_string().into())).await;
                let _ = ws.send(Message::Close(None)).await;
                return true;
            }
            if close || hold {
                let _ = ws.send(Message::Close(None)).await;
                if hold {
                    while let Some(Ok(incoming)) = ws.next().await {
                        if matches!(incoming, Message::Close(_)) { break; }
                    }
                }
                return true;
            }
            phx_reply(&msg, json!({"diff": {"s": [r#"<form phx-trigger-action action="/auth/user/otp/sign_in" method="post"><input type="hidden" name="user[email]" value=""><input type="hidden" name="user[code]" value=""></form>"#]}}))
        }
        "event" => phx_reply(&msg, json!({"diff": {}})),
        _ => phx_reply(&msg, json!({})),
    };
    ws.send(out).await.is_err()
}

struct OpenSlot {
    ctl: Arc<Mutex<Ctl>>,
    gated: bool,
}
impl Drop for OpenSlot {
    fn drop(&mut self) {
        let mut c = self.ctl.lock().unwrap();
        c.open = c.open.saturating_sub(1);
        if self.gated { c.gated_open = c.gated_open.saturating_sub(1); }
    }
}

async fn serve() -> (String, Arc<Mutex<Ctl>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let ctl = Arc::new(Mutex::new(Ctl::default()));
    let app = Router::new()
        .route("/rs", get(site))
        .route("/auth", get(auth_page))
        .route("/auth/user/otp/sign_in", post(sign_in))
        .route("/live/websocket", get(live))
        .with_state(ctl.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, ctl)
}

fn fresh_home(name: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("acp-login-{name}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(home.join(".acp")).unwrap();
    home
}

fn session_file(home: &std::path::Path) -> std::path::PathBuf { home.join(".acp/session.json") }

async fn emailed_code() -> Result<String> {
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(CODE.into())
}

async fn begin_at(home: &std::path::Path) -> LoginSession {
    unsafe {
        std::env::set_var("HOME", home);
        std::env::set_var("ACP_HOME", home);
    }
    let jar = Arc::new(Jar::default());
    let http = client(jar.clone()).unwrap();
    match LoginSession::start(&http, &jar, Some("ada@example.test".into())).await.unwrap() {
        LoginStart::NeedCode(s) => *s,
        LoginStart::Already => panic!("a fresh jar should be logged out"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn login_heartbeats_retries_a_closed_socket_and_keeps_a_wrong_code() {
    let (base, ctl) = serve().await;
    unsafe { std::env::set_var("ACP_BASE_URL", &base); }

    // Heartbeats while the code is being read, then a normal success.
    let home = fresh_home("beat");
    let mut session = begin_at(&home).await;
    session.set_heartbeat(Duration::from_millis(40));
    let code = session.pump_until(emailed_code()).await.unwrap().unwrap();
    let events = ctl.lock().unwrap().events.clone();
    assert!(events.iter().filter(|e| **e == "hb").count() >= 2, "heartbeats during the wait: {events:?}");
    assert!(!events.contains(&"verify"), "the code is not submitted while we are still waiting");
    session.submit_code(&code).await.unwrap();
    let saved = std::fs::read_to_string(session_file(&home)).unwrap();
    assert!(saved.contains(SESSION), "{saved}");

    // A wrong code leaves the attempt usable. The next code signs in.
    let home = fresh_home("wrong");
    let mut session = begin_at(&home).await;
    let err = session.submit_code("NOPE").await.unwrap_err();
    assert!(err.to_string().contains("sign-in POST failed"), "{err:#}");
    assert!(!session_file(&home).exists(), "a rejected code does not write a session");
    session.submit_code(CODE).await.unwrap();
    assert!(session_file(&home).is_file());

    // The socket closes on the first verify. The attempt reconnects and still signs in.
    ctl.lock().unwrap().close_next_verify = true;
    let joins_before = ctl.lock().unwrap().joins;
    let home = fresh_home("close");
    let mut session = begin_at(&home).await;
    let joined = ctl.lock().unwrap().joins;
    assert!(joined > joins_before, "start joins the LiveView");
    session.submit_code(CODE).await.unwrap();
    let after = ctl.lock().unwrap().joins;
    assert!(after > joined, "a closed verify reconnects (joins {joined} -> {after})");
    assert!(!ctl.lock().unwrap().close_next_verify);
    let saved = std::fs::read_to_string(session_file(&home)).unwrap();
    assert!(saved.contains(SESSION), "{saved}");

    // `ACP_HOME` is the data root. An empty value falls back to `$HOME`.
    let isolated = fresh_home("acp-home");
    unsafe {
        std::env::set_var("HOME", "/tmp/acp-real-home-not-used");
        std::env::set_var("ACP_HOME", &isolated);
    }
    assert_eq!(acp_red_star::config::home().unwrap(), isolated);
    unsafe { std::env::set_var("ACP_HOME", ""); }
    assert_eq!(acp_red_star::config::home().unwrap(), std::path::PathBuf::from("/tmp/acp-real-home-not-used"));

    // The site renders the sign-in form, then closes the socket, and rejects a
    // blind OTP post. The client must POST the form it already rendered.
    {
        let mut c = ctl.lock().unwrap();
        c.diff_then_close = true;
        c.require_form_token = true;
        c.events.clear();
    }
    let home = fresh_home("trigger");
    let mut session = begin_at(&home).await;
    session.submit_code(CODE).await.unwrap();
    let saved = std::fs::read_to_string(session_file(&home)).unwrap();
    assert!(saved.contains(SESSION), "{saved}");
    drop(session);
    {
        let mut c = ctl.lock().unwrap();
        c.diff_then_close = false;
        c.require_form_token = false;
        c.one_at_a_time = true;
        c.hold_close = true;
        c.rejected = 0;
        c.gated_open = 0;
    }

    // The old socket stays counted until this client closes it. Reconnect must
    // close first or the new join is refused.
    let joins_before = ctl.lock().unwrap().joins;
    let home = fresh_home("one-socket");
    let mut session = begin_at(&home).await;
    session.submit_code(CODE).await.unwrap();
    let rejected = ctl.lock().unwrap().rejected;
    let joins = ctl.lock().unwrap().joins;
    assert_eq!(rejected, 0, "the new join was refused while the old socket was still open");
    assert!(joins > joins_before + 1, "verify closes the first socket and a second join finishes the login (joins {joins_before} -> {joins})");
    assert!(session_file(&home).is_file());
    ctl.lock().unwrap().one_at_a_time = false;
    ctl.lock().unwrap().hold_close = false;

    // The TUI waits on a channel, on this same runtime, and still heartbeats.
    {
        let mut c = ctl.lock().unwrap();
        c.ping_after_email = true;
        c.pongs = 0;
        c.events.clear();
    }
    let home = fresh_home("tui-task");
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut session = begin_at(&home).await;
    session.set_heartbeat(Duration::from_millis(40));
    let task = tokio::spawn(async move {
        let mut cmd_rx = cmd_rx;
        let code = session.pump_until(cmd_rx.recv()).await.map_err(|e| format!("{e:#}"))?.ok_or("login cancelled")?;
        session.submit_code(&code).await.map_err(|e| format!("{e:#}"))
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    let beats = ctl.lock().unwrap().events.iter().filter(|e| **e == "hb").count();
    assert!(beats >= 2, "the TUI wait heartbeats before the code is sent");
    cmd_tx.send(CODE.to_string()).unwrap();
    task.await.unwrap().unwrap();
    assert!(ctl.lock().unwrap().pongs >= 1, "a protocol ping during the wait is answered");
    assert!(std::fs::read_to_string(session_file(&home)).unwrap().contains(SESSION));
}
