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

async fn sign_in(body: String) -> Response {
    let ok = body.split('&').any(|kv| kv == format!("user%5Bcode%5D={CODE}") || kv == format!("user[code]={CODE}"));
    if !ok { return (StatusCode::FORBIDDEN, "bad code").into_response(); }
    (StatusCode::FOUND, [(header::SET_COOKIE, format!("{SESSION}=ok; Path=/; HttpOnly")), (header::LOCATION, "/rs".to_string())]).into_response()
}

fn phx_reply(msg: &Value, response: Value) -> Message {
    Message::Text(json!([msg[0], msg[1], msg[2], "phx_reply", {"status": "ok", "response": response}]).to_string().into())
}

async fn live(State(ctl): State<Arc<Mutex<Ctl>>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| lv_conn(ctl, socket))
}

async fn lv_conn(ctl: Arc<Mutex<Ctl>>, mut ws: WebSocket) {
    while let Some(Ok(Message::Text(t))) = ws.next().await {
        let Ok(msg) = serde_json::from_str::<Value>(t.as_str()) else { continue };
        let event = msg[3].as_str().unwrap_or("");
        if event == "heartbeat" {
            ctl.lock().unwrap().events.push("hb");
            if ws.send(phx_reply(&msg, json!({}))).await.is_err() { break; }
            continue;
        }
        let out = match event {
            "phx_join" => {
                ctl.lock().unwrap().joins += 1;
                let form = r#"<form phx-submit="send-login-email" phx-target="1"><input type="email" name="user[email]"></form>"#;
                phx_reply(&msg, json!({"rendered": {"s": [form]}}))
            }
            "event" => {
                let kind = msg[4]["event"].as_str().unwrap_or("");
                match kind {
                    "send-login-email" => phx_reply(&msg, json!({"diff": {"s": [r#"<form phx-submit="verify-code"><input type="text" name="user[code]"></form>"#]}})),
                    "verify-code" => {
                        let close = {
                            let mut c = ctl.lock().unwrap();
                            c.verifies += 1;
                            c.events.push("verify");
                            let close = c.close_next_verify;
                            if close { c.close_next_verify = false; }
                            close
                        };
                        if close {
                            let _ = ws.send(Message::Close(None)).await;
                            return;
                        }
                        phx_reply(&msg, json!({"diff": {"s": [r#"<form phx-trigger-action action="/auth/user/otp/sign_in" method="post"><input type="hidden" name="user[email]" value=""><input type="hidden" name="user[code]" value=""></form>"#]}}))
                    }
                    _ => phx_reply(&msg, json!({"diff": {}})),
                }
            }
            _ => phx_reply(&msg, json!({})),
        };
        if ws.send(out).await.is_err() { break; }
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
    unsafe { std::env::set_var("HOME", home); }
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
}
