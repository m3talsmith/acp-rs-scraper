//! Login: email + one-time code over the `/auth` LiveView, finished by the
//! sign-in POST that sets the session cookie.
//!
//! The socket stays up while the code is read: Phoenix heartbeats go out on
//! [`LV_HEARTBEAT`](crate::liveview::LV_HEARTBEAT). A heartbeat with no reply
//! before the next one is a dead socket (a successful write is not enough).
//! The old socket is closed and dropped before a new one is joined. Rejoining
//! does not send the email again. The code form seen before the drop
//! (`user[otp]`, `submit-code`, the form's own `_csrf_token`) is what the
//! fallback POST sends. If the verify diff already rendered the
//! `phx-trigger-action` sign-in form and the socket then closes, that form is
//! posted anyway: that close is the site leaving LiveView, not a failed code.
//! A redirect back to `/auth` is a rejection, and the flash text is the error.
//! A wrong code leaves the attempt open.

pub mod prefs;
pub mod session;

use crate::prelude::*;

/// Look for a redirect target anywhere in a LiveView reply.
pub fn find_redirect(v: &Value) -> Option<String> {
    for k in ["redirect", "live_redirect", "navigate"] {
        if let Some(to) = v.pointer(&format!("/response/{k}/to")).and_then(Value::as_str) {
            return Some(to.into());
        }
    }
    None
}

/// Discover the code form's event / field / cid from the rendered LiveView
/// (falls back to the raw reply text, then to constants).
pub fn detect_code_form(html: &str, reply: &Value) -> (String, String, Option<u64>) {
    let raw = reply.to_string().replace("\\\"", "\"");
    if let Some(form) = otp_form_from_html(html).or_else(|| otp_form_from_html(&raw)) {
        return (form.event, form.code_field, form.cid.or(Some(EMAIL_FORM_CID)));
    }
    (CODE_EVENT_FALLBACK.into(), CODE_FIELD_FALLBACK.into(), Some(EMAIL_FORM_CID))
}

/// `phx-trigger-action`: LiveView asks the client to do a real HTTP POST of
/// the form (action="/auth/user/otp/sign_in") so the server can set the
/// authenticated session cookie.
pub async fn trigger_action_post(
    http: &reqwest::Client, html: &str, csrf: &str, email: &str, code_field: &str, code: &str,
    return_to: Option<&str>,
) -> Result<()> {
    // Own the fields before the POST. A scraper node held across `.await` is
    // not `Send`, which forced the terminal UI onto a second runtime and closed
    // the LiveView socket on submit ("websocket closed").
    let (action, mut fields) = {
        let doc = Html::parse_fragment(html);
        let form = doc.select(&Selector::parse("form[phx-trigger-action]").unwrap()).next()
            .ok_or_else(|| anyhow!("no redirect and no phx-trigger-action form after code"))?;
        let action = form.value().attr("action").unwrap_or(OTP_SIGN_IN_FALLBACK).to_string();
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
        (action, fields)
    };
    if !fields.iter().any(|(n, _)| n == "_csrf_token") {
        fields.push(("_csrf_token".into(), csrf.into()));
    }
    if let Some(rt) = return_to.filter(|s| !s.is_empty())
        && !fields.iter().any(|(n, _)| n == "return_to")
    {
        fields.push(("return_to".into(), rt.into()));
    }
    post_sign_in(http, &action, &fields, code).await
}

/// POST the sign-in form the way the browser does, then follow redirects only
/// to see where they land. A landing on `/auth` is a rejection (the flash is
/// the error). The code and the cookie values are not logged.
async fn post_sign_in(
    http: &reqwest::Client, action: &str, fields: &[(String, String)], code: &str,
) -> Result<()> {
    let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
    verbose!("POST {action} with fields {names:?}");
    let resp = http.post(Url::parse(base_url())?.join(action)?)
        .header("Origin", base_url())
        .header("Referer", format!("{}/auth", base_url()))
        .header("Sec-Fetch-Mode", "navigate")
        .form(fields)
        .send().await?;
    let status = resp.status();
    let final_url = resp.url().clone();
    let on_login = matches!(final_url.path(), "/auth" | "/auth/");
    if on_login {
        let body = resp.text().await.unwrap_or_default();
        if let Some(f) = flash_text(&body, code) {
            bail!("sign-in was rejected: {f}");
        }
        bail!("sign-in was rejected");
    }
    if !status.is_success() {
        bail!("sign-in POST failed: {status}");
    }
    Ok(())
}

/// The code form remembered from a LiveView diff, so a later POST can use it
/// after the socket (and the form) are gone.
struct OtpForm {
    action: String,
    event: String,
    code_field: String,
    cid: Option<u64>,
    fields: Vec<(String, String)>,
}

fn code_markers(s: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "user[otp]",
        "user[code]",
        "phx-trigger-action",
        "phx-submit=\"submit-code\"",
        "phx-submit=\\\"submit-code\\\"",
        "phx-submit=\"verify-code\"",
        "phx-submit=\\\"verify-code\\\"",
    ];
    NEEDLES.iter().any(|n| s.contains(n))
}

fn return_to_from(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    u.query_pairs().find(|(k, v)| k == "return_to" && !v.is_empty()).map(|(_, v)| v.into_owned())
}

/// Alert / flash text from an `/auth` page. `code` is removed so a rejection
/// that echoes the one-time code is not logged or shown.
fn flash_text(html: &str, code: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("[role=alert], .alert, .flash, #flash").ok()?;
    let mut parts = Vec::new();
    for el in doc.select(&sel) {
        let t = el.text().collect::<Vec<_>>().join(" ");
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() { parts.push(t); }
    }
    let t = parts.join(" ");
    if t.is_empty() { return None; }
    let t = if code.is_empty() { t } else { t.replace(code, "[REDACTED]") };
    Some(t)
}

fn otp_form_from_html(html: &str) -> Option<OtpForm> {
    let doc = Html::parse_fragment(html);
    let form = doc.select(&Selector::parse("form").ok()?).find(|f| {
        let submit = f.value().attr("phx-submit").unwrap_or("");
        submit == "submit-code" || submit == "verify-code" || submit == CODE_EVENT_FALLBACK
            || f.value().attr("phx-trigger-action").is_some()
            || f.select(&Selector::parse("input[name]").unwrap())
                .any(|i| matches!(i.value().attr("name"), Some("user[otp]" | "user[code]")))
    })?;
    let event = form.value().attr("phx-submit").filter(|s| *s != EMAIL_EVENT)
        .unwrap_or(CODE_EVENT_FALLBACK).to_string();
    let action = form.value().attr("action").unwrap_or(OTP_SIGN_IN_FALLBACK).to_string();
    let cid = form.value().attr("phx-target").and_then(|s| s.parse().ok());
    let mut fields = Vec::new();
    for i in form.select(&Selector::parse("input[name]").unwrap()) {
        let name = i.value().attr("name").unwrap_or_default();
        if name.is_empty() { continue; }
        let typ = i.value().attr("type").unwrap_or("text").to_ascii_lowercase();
        if typ == "submit" || typ == "button" || typ == "image" { continue; }
        let value = i.value().attr("value").unwrap_or_default().to_string();
        if typ == "checkbox" || typ == "radio" {
            let checked = i.value().attr("checked").is_some();
            // `user[remember_me]` is part of this sign-in form. An unchecked
            // marketing box is omitted, the way a browser omits it.
            if !checked && name != "user[remember_me]" { continue; }
            let v = if value.is_empty() { "true".into() } else { value };
            fields.push((name.to_string(), v));
            continue;
        }
        fields.push((name.to_string(), value));
    }
    let code_field = fields.iter().map(|(n, _)| n.clone())
        .find(|n| n == "user[otp]" || n == "user[code]")
        .unwrap_or_else(|| CODE_FIELD_FALLBACK.into());
    Some(OtpForm { action, event, code_field, cid, fields })
}

/// Whether the jar's session is accepted by the site: `GET /rs` does not redirect to `/auth`.
pub async fn session_valid(http: &reqwest::Client) -> Result<bool> {
    let resp = http.get(start_url()).send().await?;
    Ok(!resp.url().path().starts_with("/auth"))
}

/// Whether `err` is a dropped LiveView socket (retry / reconnect) rather than a rejected code.
pub fn socket_closed(err: &anyhow::Error) -> bool {
    let s = format!("{err:#}").to_lowercase();
    ["websocket closed", "connection reset", "broken pipe", "connection closed", "already closed"]
        .iter().any(|n| s.contains(n))
}

/// Result of [`LoginSession::start`].
pub enum LoginStart {
    /// `GET /rs` did not redirect to `/auth`. The session file was saved.
    Already,
    /// The email was accepted. The socket is joined and waiting for the code.
    NeedCode(Box<LoginSession>),
}

/// One login attempt. The socket is kept alive while the code is read, and a
/// close reconnects instead of dropping the attempt.
pub struct LoginSession {
    http: reqwest::Client,
    jar: Arc<Jar>,
    email: String,
    page: LvPage,
    /// `None` only while a reconnect has dropped the old socket and not joined the new one,
    /// or after the site reported that this jar is already signed in.
    lv: Option<Lv>,
    email_reply: Value,
    /// Code form from the diff that followed the email. Kept after the socket
    /// dies so the fallback POST still has the form's csrf and fields.
    code_form: Option<OtpForm>,
    /// `return_to` query on the `/auth` page, sent with the sign-in POST.
    return_to: Option<String>,
    /// `GET /auth` came back already signed in (the socket had died after the cookie was set).
    signed_in: bool,
}

impl LoginSession {
    /// Open `/rs`. When it redirects to `/auth`, submit `email` (or the saved / prompted one).
    pub async fn start(http: &reqwest::Client, jar: &Arc<Jar>, email: Option<String>) -> Result<LoginStart> {
        let resp = http.get(start_url()).send().await?;
        let url = resp.url().to_string();
        let html = resp.text().await?;
        if !url.contains("/auth") {
            info!("Already logged in.");
            save_session(jar)?;
            return Ok(LoginStart::Already);
        }
        let page = parse_lv(&url, &html)?;
        let return_to = return_to_from(&url);
        let email = match email {
            Some(e) => { save_email(&e)?; e }
            None => load_email()?,
        };
        let mut lv = Lv::connect(&page, jar).await?;
        let email_reply = submit_email(&mut lv, &email).await?;
        info!("Login email requested for {email}.");
        let mut session = LoginSession {
            http: http.clone(), jar: jar.clone(), email, page, lv: Some(lv), email_reply,
            code_form: None, return_to, signed_in: false,
        };
        session.note_code_form();
        Ok(LoginStart::NeedCode(Box::new(session)))
    }

    /// Heartbeat interval for this attempt. `0` is treated as 1ms. Tests set a short interval.
    pub fn set_heartbeat(&mut self, every: std::time::Duration) {
        if let Some(lv) = self.lv.as_mut() {
            lv.heartbeat = if every.is_zero() { std::time::Duration::from_millis(1) } else { every };
        }
    }

    /// Poll `fut` while heartbeating. A dropped socket reconnects and `fut` keeps running.
    pub async fn pump_until<T>(&mut self, fut: impl std::future::Future<Output = T>) -> Result<T> {
        if self.signed_in { return Ok(fut.await); }
        if self.lv.is_none() {
            self.reconnect().await?;
            if self.signed_in { return Ok(fut.await); }
        }
        self.note_code_form();
        let mut fut = std::pin::pin!(fut);
        let every = self.lv.as_ref().map(|l| l.heartbeat).unwrap_or(LV_HEARTBEAT);
        let mut beat = tokio::time::interval(every);
        beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        beat.tick().await;
        let mut drops = 0u8;
        loop {
            enum Ev<T> { Done(T), Beat, Frame(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>) }
            let ev = {
                let lv = self.lv.as_mut().context("login socket is closed")?;
                let ws = &mut lv.ws;
                tokio::select! {
                    biased;
                    out = &mut fut => Ev::Done(out),
                    msg = ws.next() => Ev::Frame(msg),
                    _ = beat.tick() => Ev::Beat,
                }
            };
            match ev {
                Ev::Done(out) => return Ok(out),
                Ev::Beat => {
                    // A write is not a reply. `send_heartbeat` fails when the
                    // previous heartbeat is still unanswered.
                    let beat_result = match self.lv.as_mut() {
                        Some(lv) => lv.send_heartbeat().await,
                        None => Err(anyhow!("login socket is closed")),
                    };
                    if let Err(e) = beat_result { self.recover(e, &mut drops).await?; }
                }
                Ev::Frame(msg) => {
                    let absorbed = match self.lv.as_mut() {
                        Some(lv) => lv.absorb(msg, None).await,
                        None => Err(anyhow!("login socket is closed")),
                    };
                    if let Err(e) = absorbed {
                        self.recover(e, &mut drops).await?;
                    } else {
                        drops = 0;
                        self.note_code_form();
                    }
                }
            }
        }
    }

    /// Submit the emailed code (or a login link) and save `~/.acp/session.json`.
    /// A closed socket reconnects and the verify step is tried again, up to three times.
    /// A rejected code is returned as-is and this attempt stays usable.
    pub async fn submit_code(&mut self, code: &str) -> Result<()> {
        let code = code.trim();
        if code.is_empty() { bail!("paste the code (or the login link) from the email"); }
        if self.signed_in || session_valid(&self.http).await? {
            return self.confirm_session().await;
        }
        if code.starts_with("http") {
            self.http.get(code).send().await?.error_for_status()?;
            return self.confirm_session().await;
        }
        let mut last = None;
        for attempt in 0..3 {
            match self.verify_once(code).await {
                Ok(()) => return self.confirm_session().await,
                Err(e) if socket_closed(&e) && attempt + 1 < 3 => {
                    last = Some(e);
                    self.reconnect().await?;
                    if self.signed_in { return self.confirm_session().await; }
                }
                Err(e) => return Err(e),
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("websocket closed")))
    }

    /// Ask for another code on this same attempt. Reconnects first when the socket is gone.
    pub async fn resend(&mut self) -> Result<()> {
        if self.signed_in { return Ok(()); }
        match self.submit_email_here().await {
            Ok(()) => Ok(()),
            Err(e) => {
                if !socket_closed(&e) { verbose!("resend failed ({e:#}); refreshing the login"); }
                self.reconnect().await?;
                if self.signed_in { return Ok(()); }
                self.submit_email_here().await
            }
        }
    }

    async fn submit_email_here(&mut self) -> Result<()> {
        let lv = self.lv.as_mut().context("login socket is closed")?;
        self.email_reply = submit_email(lv, &self.email).await?;
        info!("Login email requested again for {}.", self.email);
        Ok(())
    }

    async fn verify_once(&mut self, code: &str) -> Result<()> {
        self.note_code_form();
        if !self.ready_for_code() {
            return self.direct_otp_post(code).await;
        }
        let html = self.lv.as_ref().context("login socket is closed")?.html();
        let (ev, field, cid) = match &self.code_form {
            Some(form) => (form.event.clone(), form.code_field.clone(), form.cid.or(Some(EMAIL_FORM_CID))),
            None => detect_code_form(&html, &self.email_reply),
        };
        let replied = match self.lv.as_mut().context("login socket is closed")?.form(&ev, &field, code, cid).await {
            Ok(r) => r,
            Err(e) if socket_closed(&e) && self.html_has_trigger() => {
                // The diff rendered the sign-in form, then the socket closed.
                // The browser disconnects and POSTs that form; do the same.
                return self.finish_trigger(&field, code).await;
            }
            Err(e) => return Err(e),
        };
        if replied["status"] != "ok" { bail!("code submit failed: {replied}"); }
        self.note_code_form();
        if let Some(to) = find_redirect(&replied) {
            self.http.get(Url::parse(base_url())?.join(&to)?).send().await?.error_for_status()?;
        } else if self.html_has_trigger() {
            self.finish_trigger(&field, code).await?;
        } else {
            self.direct_otp_post(code).await?;
        }
        Ok(())
    }

    fn html_has_trigger(&self) -> bool {
        self.lv.as_ref().is_some_and(|lv| lv.html().contains("phx-trigger-action") || lv.rendered.to_string().contains("phx-trigger-action"))
    }

    async fn finish_trigger(&self, field: &str, code: &str) -> Result<()> {
        let html = self.lv.as_ref().context("login socket is closed")?.html();
        trigger_action_post(&self.http, &html, &self.page.csrf, &self.email, field, code, self.return_to.as_deref()).await
    }

    /// The socket no longer has the code form (a re-join shows the email form,
    /// and sending the email again would mail a new code). Post the form that
    /// was rendered when the code was requested.
    async fn direct_otp_post(&self, code: &str) -> Result<()> {
        let (action, code_field, mut fields) = match &self.code_form {
            Some(form) => (form.action.clone(), form.code_field.clone(), form.fields.clone()),
            None => (
                OTP_SIGN_IN_FALLBACK.to_string(),
                CODE_FIELD_FALLBACK.to_string(),
                vec![
                    ("_csrf_token".into(), String::new()),
                    (EMAIL_FIELD.into(), String::new()),
                    (CODE_FIELD_FALLBACK.into(), String::new()),
                ],
            ),
        };
        let mut saw_csrf = false;
        let mut saw_email = false;
        let mut saw_code = false;
        let mut saw_return = false;
        for (n, v) in &mut fields {
            if n == "_csrf_token" {
                saw_csrf = true;
                if v.is_empty() { *v = self.page.csrf.clone(); }
            } else if n == EMAIL_FIELD {
                saw_email = true;
                if v.is_empty() { *v = self.email.clone(); }
            } else if n == &code_field || n == "user[otp]" || n == "user[code]" {
                saw_code = true;
                *v = code.to_string();
            } else if n == "return_to" {
                saw_return = true;
            }
        }
        if !saw_csrf { fields.push(("_csrf_token".into(), self.page.csrf.clone())); }
        if !saw_email { fields.push((EMAIL_FIELD.into(), self.email.clone())); }
        if !saw_code { fields.push((code_field, code.to_string())); }
        if !saw_return && let Some(rt) = &self.return_to {
            fields.push(("return_to".into(), rt.clone()));
        }
        verbose!("POST {action} directly (the LiveView code form is not on this socket)");
        post_sign_in(&self.http, &action, &fields, code).await
    }

    /// True when the joined view is showing the code form. The live form is
    /// `user[otp]` / `submit-code`; `user[code]` / `verify-code` is the older
    /// shape. The rendered tree is checked as well as the HTML, because a
    /// keyed comprehension can hold the form in `k` / `p`.
    fn ready_for_code(&self) -> bool {
        let Some(lv) = self.lv.as_ref() else { return false };
        code_markers(&lv.html()) || code_markers(&lv.rendered.to_string())
    }

    /// Remember the code form while it is on the socket. A later render that
    /// is only the email form does not wipe it.
    fn note_code_form(&mut self) {
        let Some(lv) = self.lv.as_ref() else { return };
        let html = lv.html();
        let raw = lv.rendered.to_string().replace("\\\"", "\"");
        if let Some(form) = otp_form_from_html(&html).or_else(|| otp_form_from_html(&raw)) {
            self.code_form = Some(form);
        }
    }

    async fn confirm_session(&mut self) -> Result<()> {
        if !session_valid(&self.http).await? { bail!("still not logged in after verification"); }
        info!("Logged in.");
        save_session(&self.jar)?;
        self.signed_in = true;
        Ok(())
    }

    async fn recover(&mut self, e: anyhow::Error, drops: &mut u8) -> Result<()> {
        if !socket_closed(&e) { return Err(e); }
        *drops += 1;
        if *drops > 3 { return Err(e); }
        verbose!("login socket closed; reconnecting");
        self.reconnect().await
    }

    async fn reconnect(&mut self) -> Result<()> {
        let every = self.lv.as_ref().map(|l| l.heartbeat).unwrap_or(LV_HEARTBEAT);
        // Drop the old socket before dialing. Holding it open while the new
        // join is in flight makes the server close the new one ("websocket closed").
        if let Some(mut old) = self.lv.take() {
            old.close().await;
            drop(old);
        }
        let auth = match &self.return_to {
            Some(rt) if !rt.is_empty() => format!("{}/auth?return_to={}", base_url(), urlenc(rt)),
            _ => format!("{}/auth", base_url()),
        };
        let resp = self.http.get(auth).send().await.context("reconnecting the login")?;
        let url = resp.url().to_string();
        let html = resp.text().await?;
        if !url.contains("/auth") {
            self.signed_in = true;
            return Ok(());
        }
        if let Some(rt) = return_to_from(&url) { self.return_to = Some(rt); }
        self.page = parse_lv(&url, &html)?;
        let mut lv = Lv::connect(&self.page, &self.jar).await?;
        lv.heartbeat = every;
        self.lv = Some(lv);
        // A restored code form (new csrf included) replaces the snapshot.
        // The email form does not: the code already mailed must stay usable.
        self.note_code_form();
        Ok(())
    }
}

async fn submit_email(lv: &mut Lv, email: &str) -> Result<Value> {
    let r = lv.form(EMAIL_EVENT, EMAIL_FIELD, email, Some(EMAIL_FORM_CID)).await?;
    if r["status"] != "ok" { bail!("email submit failed: {r}"); }
    Ok(r)
}

/// Log in: open `/rs` (redirects to `/auth` when logged out), submit the saved email over the LiveView, prompt for the one-time code (or login link), submit it and finish with the sign-in POST. Fails if `/rs` still redirects to `/auth`. A saved session in the jar (see [`session`]) skips the code; the session is saved again on success.
///
/// The prompt runs on a blocking thread so this task can heartbeat the LiveView socket while the code is typed.
pub async fn login(http: &reqwest::Client, jar: &Arc<Jar>) -> Result<()> {
    login_flow(http, jar, None, || async {
        tokio::task::spawn_blocking(|| prompt("Verification code (or paste the login link): "))
            .await
            .context("reading the verification code")?
    }).await
}

/// The login steps behind [`login`], with the email and the code supplied by the caller
/// (the terminal UI's Login modal passes the typed email and awaits the typed code).
/// `email = None` reads it from `~/.acp/preferences.toml` (prompting when missing);
/// `Some` is used as given and saved there. `code` is only called when the site asks.
/// Heartbeats run while `code` is pending. A dropped socket reconnects before the code is submitted.
pub async fn login_flow<F, Fut>(http: &reqwest::Client, jar: &Arc<Jar>, email: Option<String>, code: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let LoginStart::NeedCode(session) = LoginSession::start(http, jar, email).await? else { return Ok(()) };
    let mut session = *session;
    let code = session.pump_until(code()).await??;
    session.submit_code(&code).await
}

#[cfg(test)]
mod tests {
    use super::socket_closed;

    #[test]
    fn login_socket_errors_are_retries_and_a_bad_code_is_not() {
        assert!(socket_closed(&anyhow::anyhow!("websocket closed")));
        assert!(socket_closed(&anyhow::anyhow!("websocket closed: connection reset by peer")));
        assert!(socket_closed(&anyhow::anyhow!("broken pipe")));
        assert!(!socket_closed(&anyhow::anyhow!("sign-in POST failed: 403 Forbidden")));
        assert!(!socket_closed(&anyhow::anyhow!("still not logged in after verification")));
    }

    #[test]
    fn login_session_can_move_to_the_app_runtime() {
        fn assert_send<T: Send>() {}
        assert_send::<super::LoginSession>();
    }

    #[test]
    fn code_form_snapshot_uses_the_diff_token_and_otp_field() {
        let html = r#"<form action="/auth/user/otp/sign_in" method="post" phx-target="1" phx-submit="submit-code"><input type="hidden" name="_csrf_token" value="form-csrf"><input type="hidden" name="user[email]" value=""><input type="text" name="user[otp]"><input type="hidden" name="user[remember_me]" value="true"><input type="hidden" name="marketing_opt_in" value="false"></form>"#;
        let form = super::otp_form_from_html(html).expect("code form");
        assert_eq!(form.event, "submit-code");
        assert_eq!(form.code_field, "user[otp]");
        assert_eq!(form.cid, Some(1));
        assert!(form.fields.iter().any(|(n, v)| n == "_csrf_token" && v == "form-csrf"));
        assert!(form.fields.iter().any(|(n, v)| n == "user[remember_me]" && v == "true"));
        assert!(form.fields.iter().any(|(n, v)| n == "marketing_opt_in" && v == "false"));
        assert!(super::code_markers(html));
        assert!(!super::code_markers(r#"<form phx-submit="send-login-email"><input type="email" name="user[email]"></form>"#));
        let older = r#"<form phx-submit="verify-code"><input type="text" name="user[code]"></form>"#;
        let older = super::otp_form_from_html(older).expect("older code form");
        assert_eq!(older.event, "verify-code");
        assert_eq!(older.code_field, "user[code]");
    }

    #[test]
    fn auth_flash_is_the_rejection_and_the_code_is_not_kept() {
        let html = r#"<html><body><p class="alert alert-danger" role="alert">invalid or expired code</p></body></html>"#;
        assert_eq!(super::flash_text(html, "KWMRUV").as_deref(), Some("invalid or expired code"));
        let echoed = r#"<p role="alert">KWMRUV is not valid</p>"#;
        assert_eq!(super::flash_text(echoed, "KWMRUV").as_deref(), Some("[REDACTED] is not valid"));
    }
}
